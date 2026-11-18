use serde_json::{Value, json};

use super::Calling;
use crate::agent::Agent;
use crate::tools::names::ToolName;
use crate::tools::subagent::AgentTool;
use crate::tools::{Consult, Outcome, Tool};
use crate::transcript::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Handler {
    TaskOutput,
    TaskStop,
    TaskList,
    Ask,
    ExitPlan,
    EnterPlan,
    EnterWorktree,
    ExitWorktree,
    Agent,
    SendMessage,
    Watch,
    Repl,
    Tool,
}

impl Handler {
    pub(super) fn of(name: &str) -> Self {
        match ToolName::of(name) {
            ToolName::TaskOutput => Self::TaskOutput,
            ToolName::TaskStop => Self::TaskStop,
            ToolName::TaskList => Self::TaskList,
            ToolName::AskUserQuestion => Self::Ask,
            ToolName::ExitPlanMode => Self::ExitPlan,
            ToolName::EnterPlanMode => Self::EnterPlan,
            ToolName::EnterWorktree => Self::EnterWorktree,
            ToolName::ExitWorktree => Self::ExitWorktree,
            ToolName::Agent => Self::Agent,
            ToolName::SendMessage => Self::SendMessage,
            ToolName::Watch => Self::Watch,
            ToolName::Repl => Self::Repl,
            _ => Self::Tool,
        }
    }

    pub(super) fn run_by_the_loop(self) -> bool {
        !matches!(self, Self::Tool | Self::Repl)
    }

    pub(super) fn starts_background_work(self) -> bool {
        matches!(self, Self::Agent | Self::Repl)
    }
}

pub(super) fn not_alongside(name: &str) -> Outcome {
    Outcome::error(format!(
        "{name} cannot start background work from a group of calls run side by side. Call it on its own."
    ))
}

impl Agent<'_> {
    pub(super) fn in_loop<'env>(&'env self, calling: &Calling<'_, 'env>, input: &Value) -> Outcome {
        let Calling { id, name, tool, run, parent, .. } = *calling;
        let handler = Handler::of(name);
        match handler {
            Handler::TaskOutput => return run.background.output(input),
            Handler::TaskStop => return run.background.stop(input),
            Handler::TaskList => return run.background.list(|| self.planned_tasks(tool, input)),
            _ => {}
        }
        if let Some(outcome) = tool.and_then(|tool| self.consulted(tool, input, parent)) {
            return outcome;
        }
        let Some(tool) = tool else { return Outcome::error(format!("There is no tool named {name}.")) };
        match handler {
            Handler::Ask => self.ask_user(id, input),
            Handler::ExitPlan => self.exit_plan_mode(id, input),
            Handler::EnterPlan => self.enter_plan_mode(),
            Handler::EnterWorktree => self.enter_worktree(run),
            Handler::ExitWorktree => self.exit_worktree(input, run),
            Handler::Agent => self.agent_tool(calling, input),
            Handler::SendMessage => self.message(calling, tool, input),
            Handler::Watch => self.watch(id, input, run),
            Handler::Repl => match calling.spawn {
                Some(spawn) => self.repl(id, input, run, parent, spawn),
                None => not_alongside(name),
            },
            Handler::TaskOutput | Handler::TaskStop | Handler::TaskList | Handler::Tool => {
                self.tool_of_its_own(calling, tool, input)
            }
        }
    }

    fn planned_tasks(&self, tool: Option<&dyn Tool>, input: &Value) -> Value {
        tool.map(|tool| tool.run(input, self.workspace))
            .and_then(|out| serde_json::from_str::<Value>(&out.content).ok())
            .unwrap_or_else(|| json!([]))
    }

    fn consulted(&self, tool: &dyn Tool, input: &Value, parent: &[Message]) -> Option<Outcome> {
        let agent =
            Consult { api: self.api, request: &self.request, conversation: parent, stop: self.control.stop_flag() };
        tool.consult(input, self.workspace, &agent)
    }

    fn agent_tool<'env>(&'env self, calling: &Calling<'_, 'env>, input: &Value) -> Outcome {
        let Calling { name, run, parent, .. } = *calling;
        if let Some(started) = self.teammate(input) {
            return started;
        }
        if !(AgentTool { agents: self.agents.to_vec() }).background(input) {
            return self.sub_agent_with(input, self.control, self.workspace, parent, &run.conversations, None);
        }
        match calling.spawn {
            Some(spawn) => self.start_background(input, run, parent, spawn),
            None => not_alongside(name),
        }
    }

    fn message(&self, calling: &Calling<'_, '_>, tool: &dyn Tool, input: &Value) -> Outcome {
        if let Some(sent) = self.team_message(input) {
            return sent;
        }
        let conversations = &calling.run.conversations;
        if !self.to_session(input, conversations) {
            return self.send_message(input, conversations);
        }
        let summary = tool.summary(input);
        self.permitted(calling.id, &crate::tools::peers::SessionMessage, &summary, input, calling.permission)
    }

    fn tool_of_its_own(&self, calling: &Calling<'_, '_>, tool: &dyn Tool, input: &Value) -> Outcome {
        let summary = tool.summary(input);
        match (self.project_skill(tool, input), tool.script()) {
            (Some(skill), _) => self.skill_call(calling, tool, skill, &summary, input),
            (None, Some(script)) => self.script_call(calling, script, &summary, input),
            (None, None) => {
                let shown = tool.shown(input, self.workspace);
                self.permitted_as(calling.id, tool, &summary, shown.as_deref(), input, calling.permission)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::names;

    #[test]
    fn each_tool_the_loop_runs_itself_has_its_handler_and_every_other_runs_as_a_tool() {
        assert_eq!(Handler::of(names::AGENT), Handler::Agent);
        assert_eq!(Handler::of(names::REPL), Handler::Repl);
        assert_eq!(Handler::of(names::TASK_LIST), Handler::TaskList);
        assert_eq!(Handler::of(names::SEND_MESSAGE), Handler::SendMessage);
        for name in [names::READ, names::BASH, names::SKILL, names::TASK_CREATE, "mcp__docs__read"] {
            assert_eq!(Handler::of(name), Handler::Tool, "{name}");
        }
    }

    #[test]
    fn only_the_agent_and_script_tools_start_background_work() {
        let all = [
            Handler::TaskOutput,
            Handler::TaskStop,
            Handler::TaskList,
            Handler::Ask,
            Handler::ExitPlan,
            Handler::EnterPlan,
            Handler::EnterWorktree,
            Handler::ExitWorktree,
            Handler::Agent,
            Handler::SendMessage,
            Handler::Watch,
            Handler::Repl,
            Handler::Tool,
        ];
        let starting: Vec<Handler> = all.into_iter().filter(|handler| handler.starts_background_work()).collect();
        assert_eq!(starting, [Handler::Agent, Handler::Repl]);
    }
}
