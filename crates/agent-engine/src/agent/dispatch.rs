use domain::{EngineEvent, ToolCall};
use serde_json::{Value, json};

use super::tasks::Tasks;
use super::{Agent, SHOWN_OUTPUT};
use crate::tools::ask::ASK_TOOL;
use crate::tools::plan_mode::{ENTER_PLAN_TOOL, EXIT_PLAN_TOOL};
use crate::tools::send_message::SEND_MESSAGE_TOOL;
use crate::tools::subagent::{AGENT_TOOL, AgentTool};
use crate::tools::worktree::{ENTER_WORKTREE, EXIT_WORKTREE};
use crate::tools::{Outcome, clip};
use crate::transcript::{Block, Message};

impl Agent<'_> {
    #[allow(clippy::needless_lifetimes)]
    pub(super) fn call<'env>(
        &'env self,
        id: &str,
        name: &str,
        input: &Value,
        tasks: &Tasks,
        parent: &[Message],
        spawn: &dyn Fn(Box<dyn FnOnce() + Send + 'env>),
    ) -> Block {
        let tool = self.tools.iter().find(|tool| tool.spec().name == name);
        let offered = self.request.tools.iter().any(|spec| spec.name == name)
            || self.workspace.deferred.as_ref().is_some_and(|deferred| deferred.is_loaded(name));
        let pre = if tool.is_some() && offered && !self.control.is_stopped() {
            self.workspace.hooks.run_with_model(
                "PreToolUse",
                name,
                json!({"tool_name":name,"tool_use_id":id,"tool_input":input}),
                self.workspace,
                Some((self.api, &self.request)),
            )
        } else {
            Ok(Default::default())
        };
        let pre = pre.unwrap_or_else(|error| crate::hooks::Decision { blocked: Some(error), ..Default::default() });
        let input = pre.updated_input.as_ref().unwrap_or(input);
        let mut rule_context = Vec::new();
        if pre.updated_input.is_some() {
            let path =
                input["file_path"].as_str().or_else(|| input["path"].as_str()).map(|p| self.workspace.resolve(p));
            let mut seen = self.workspace.seen_rules.lock().expect("project rules");
            for rule in &self.workspace.rules {
                match rule.applies(&self.workspace.cwd, path.as_deref()) {
                    Ok(true) if !rule.patterns.is_empty() && seen.insert(rule.path.clone()) => {
                        rule_context.push(rule.text.clone())
                    }
                    Err(error) => rule_context.push(error),
                    _ => {}
                }
            }
        }
        let summary = tool.map(|tool| tool.summary(input)).unwrap_or_default();
        (self.emit)(EngineEvent::ToolStarted(ToolCall::new(id, name, summary.clone())));
        let mut outcome = if let Some(reason) = &pre.blocked {
            Outcome::error(format!("Hook blocked {name}: {reason}"))
        } else if !rule_context.is_empty() {
            Outcome::error(format!(
                "Hook changed input. Load these project instructions and reissue the tool call: {}",
                rule_context.join("\n")
            ))
        } else if tool.is_some() && !offered {
            Outcome::error(format!(
                "Tool {name} is not available to this agent (load it with ToolSearch if listed there)."
            ))
        } else if self.control.is_stopped() {
            Outcome::error("The run was cancelled.")
        } else if name == "TaskOutput" {
            tasks.output(input)
        } else if name == "TaskStop" {
            tasks.stop(input)
        } else if name == "TaskList" {
            tasks.list(|| {
                tool.map(|tool| tool.run(input, self.workspace))
                    .and_then(|out| serde_json::from_str::<Value>(&out.content).ok())
                    .unwrap_or_else(|| json!([]))
            })
        } else if let Some(outcome) = tool.and_then(|tool| {
            let agent = crate::tools::Consult {
                api: self.api,
                request: &self.request,
                conversation: parent,
                stop: self.control.stop_flag(),
            };
            tool.consult(input, self.workspace, &agent)
        }) {
            outcome
        } else {
            match tool {
                None => Outcome::error(format!("There is no tool named {name}.")),
                Some(_) if name == ASK_TOOL => self.ask_user(id, input),
                Some(_) if name == EXIT_PLAN_TOOL => self.exit_plan_mode(id, input),
                Some(_) if name == ENTER_PLAN_TOOL => self.enter_plan_mode(),
                Some(_) if name == ENTER_WORKTREE => self.enter_worktree(tasks),
                Some(_) if name == EXIT_WORKTREE => self.exit_worktree(input, tasks),
                Some(_) if name == AGENT_TOOL && (AgentTool { agents: self.agents.to_vec() }).background(input) => {
                    self.start_background(input, tasks, parent, spawn)
                }
                Some(_) if name == AGENT_TOOL => {
                    self.sub_agent_with(input, self.control, self.workspace, parent, &tasks.conversations, None)
                }
                Some(_) if name == SEND_MESSAGE_TOOL && self.to_session(input, &tasks.conversations) => {
                    let message = crate::tools::peers::SessionMessage;
                    self.permitted(id, &message, &summary, input, pre.permission.as_deref())
                }
                Some(_) if name == SEND_MESSAGE_TOOL => self.send_message(input, &tasks.conversations),
                Some(tool) if tool.script().is_some() => match tool.script() {
                    Some(script) => self.script_call(id, script, &summary, input, parent, pre.permission.as_deref()),
                    None => self.permitted(id, tool.as_ref(), &summary, input, pre.permission.as_deref()),
                },
                Some(tool) => self.permitted(id, tool.as_ref(), &summary, input, pre.permission.as_deref()),
            }
        };
        if !pre.context.is_empty() {
            outcome.content.push_str(&format!("\nHook context: {}", pre.context));
        }
        let event = if outcome.is_error { "PostToolUseFailure" } else { "PostToolUse" };
        match self.workspace.hooks.run_with_model(
            event,
            name,
            json!({"tool_name":name,"tool_use_id":id,"tool_input":input,"tool_response":outcome.content}),
            self.workspace,
            Some((self.api, &self.request)),
        ) {
            Ok(hook) => {
                if let Some(output) = hook.updated_output {
                    outcome.content = output.as_str().map(str::to_owned).unwrap_or_else(|| output.to_string());
                    outcome.images.clear();
                }
                if let Some(reason) = hook.blocked {
                    outcome.is_error = true;
                    outcome.content.push_str(&format!("\nHook feedback: {reason}"));
                }
                outcome.content.push_str(&hook.context);
            }
            Err(error) => {
                outcome.is_error = true;
                outcome.content.push_str(&format!("\nHook failed: {error}"));
            }
        }
        if matches!(name, "TaskCreate" | "TaskUpdate") && !outcome.is_error {
            self.emit_tasks();
        }
        (self.emit)(EngineEvent::ToolFinished { id: id.to_owned() });
        (self.emit)(EngineEvent::ToolResult {
            id: id.to_owned(),
            output: clip(outcome.shown.as_deref().unwrap_or(&outcome.content), SHOWN_OUTPUT),
            failed: outcome.is_error,
        });
        if outcome.images.is_empty() {
            Block::ToolResult { id: id.to_owned(), content: outcome.content, is_error: outcome.is_error }
        } else {
            Block::ToolResultRich {
                id: id.to_owned(),
                content: outcome.content,
                images: outcome.images,
                is_error: outcome.is_error,
            }
        }
    }
}
