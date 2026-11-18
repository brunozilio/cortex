pub(in crate::agent) mod background;
pub(in crate::agent) mod conversations;
mod model;
mod report;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use domain::{AgentStatus, EngineEvent};
use platform::sync::Locked;
use serde_json::{Value, json};

use self::conversations::{Conversations, Kept, Resume};
use super::run::ThisRun;
use super::{Agent, Ending, MAX_TURNS};
use crate::agent_runtime::WorktreeLease;
use crate::control::Control;
use crate::model::{ModelError, ToolSpec, TurnRequest};
use crate::tools::names::{self, ToolName};
use crate::tools::subagent::{self, AgentTool, FoundAgent};
use crate::tools::subagent_config::AgentConfig;
use crate::tools::{Outcome, Workspace, builtin_agents};
use crate::transcript::{Message, Usage};

fn top_level_only(name: &str) -> bool {
    matches!(
        ToolName::of(name),
        ToolName::Agent
            | ToolName::SendMessage
            | ToolName::ExitPlanMode
            | ToolName::EnterPlanMode
            | ToolName::EnterWorktree
            | ToolName::ExitWorktree
    )
}

struct Spawning<'s> {
    input: &'s Value,
    control: &'s Control,
    workspace: &'s Workspace,
    parent: &'s [Message],
    conversations: &'s Conversations,
}

struct Chosen {
    agent: Option<FoundAgent>,
    forked: bool,
}

impl Chosen {
    fn name(&self) -> &str {
        self.agent.as_ref().map_or(subagent::GENERAL, |agent| agent.name.as_str())
    }
}

struct Planned {
    config: AgentConfig,
    model: String,
    unserved: Option<String>,
    unisolated: bool,
    turn_limit: usize,
}

struct Lane {
    index: usize,
    kind: String,
    summary: String,
}

struct Ran {
    outcome: Result<Usage, Ending>,
    history: Vec<Message>,
    own: usize,
    cleanup: Result<Option<PathBuf>, String>,
    seconds: u32,
    unfinished: String,
}

impl Agent<'_> {
    pub(in crate::agent) fn sub_agent_with(
        &self,
        input: &Value,
        control: &Control,
        workspace: &Workspace,
        parent: &[Message],
        conversations: &Conversations,
        resume: Option<Resume>,
    ) -> Outcome {
        let spawning = Spawning { input, control, workspace, parent, conversations };
        self.sub_agent(&spawning, resume).unwrap_or_else(|refused| refused)
    }

    fn sub_agent(&self, spawning: &Spawning<'_>, resume: Option<Resume>) -> Result<Outcome, Outcome> {
        let chosen = self.chosen(spawning, resume.is_some())?;
        let planned = self.planned(&chosen, spawning)?;
        let child_control = spawning.control.child_configured(planned.config.permission, planned.config.dont_ask);
        let mut child_workspace = self.child_workspace(&chosen, &planned, &child_control, spawning.workspace);
        let run = ThisRun::default();
        let (mut runtime, system) = self.settled(&chosen, &planned, &mut child_workspace, &run)?;
        let task = task(spawning.input, resume.as_ref())?;
        let lane = self.announced(&chosen, spawning.input, &task, resume.as_ref());
        let saying = Mutex::new(String::new());
        let relay = |event: EngineEvent| self.relayed(lane.index, &saying, event);
        let started = std::time::Instant::now();
        let planning = child_control.mode() == domain::PermissionMode::Plan;
        let child = Agent {
            api: self.api,
            tools: self.tools,
            workspace: &child_workspace,
            control: &child_control,
            emit: &relay,
            request: TurnRequest {
                model: planned.model.clone(),
                system,
                tools: self.child_tools(&planned.config, planning, &run),
                effort: planned.config.effort.unwrap_or(self.request.effort),
                ..self.request.clone()
            },
            agents: &[],
            depth: self.depth + 1,
            turn_limit: planned.turn_limit,
        };
        let resumed = resume.is_some();
        let mut history = first_history(resume, chosen.forked, spawning.parent, &task);
        let own = history.len();
        let outcome = started_then_run(&child, &run, &lane, &task, resumed, &mut history);
        let cleanup = runtime.finish();
        let seconds = u32::try_from(started.elapsed().as_secs()).unwrap_or(u32::MAX);
        let unfinished = std::mem::take(&mut *saying.locked());
        let ran = Ran { outcome, history, own, cleanup, seconds, unfinished };
        Ok(self.reported(ran, lane, &planned, spawning))
    }

    fn chosen(&self, spawning: &Spawning<'_>, resumed: bool) -> Result<Chosen, Outcome> {
        if self.depth > 0 {
            return Err(Outcome::error("A sub-agent cannot start other sub-agents; do the work yourself."));
        }
        let known = AgentTool { agents: self.agents.to_vec() };
        let agent = known.chosen(spawning.input).map_err(Outcome::error)?.cloned();
        let forked = agent.as_ref().is_some_and(|agent| {
            agent.name == builtin_agents::FORK && agent.instructions == builtin_agents::fork_preamble()
        });
        if forked && spawning.parent.is_empty() && !resumed {
            return Err(Outcome::error("A fork needs the conversation it forks from; none was given."));
        }
        Ok(Chosen { agent, forked })
    }

    fn planned(&self, chosen: &Chosen, spawning: &Spawning<'_>) -> Result<Planned, Outcome> {
        let (input, workspace) = (spawning.input, spawning.workspace);
        let mut config = chosen.agent.as_ref().map(|agent| agent.config.clone()).unwrap_or_default();
        let kind = chosen.agent.as_ref().map_or("", |agent| agent.name.as_str());
        let picked = model::choose(kind, config.model.take(), input, &self.request.model, workspace);
        config.model = picked.model;
        if input.get("isolation").and_then(Value::as_str) == Some("worktree") {
            config.worktree = true;
        }
        let unisolated = match config.worktree.then(|| crate::agent_runtime::can_isolate(workspace)) {
            Some(Ok(false)) => true,
            Some(Err(error)) => return Err(Outcome::error(format!("Agent setup failed: {error}"))),
            Some(Ok(true)) | None => false,
        };
        if unisolated {
            config.worktree = false;
        }
        let model = config.model(&self.request.model, self.workspace.models.model.provider).map_err(Outcome::error)?;
        let turn_limit = asked_turns(input)?.min(config.max_turns.unwrap_or(MAX_TURNS));
        Ok(Planned { config, model, unserved: picked.unserved, unisolated, turn_limit })
    }

    fn child_workspace(
        &self,
        chosen: &Chosen,
        planned: &Planned,
        child_control: &Arc<Control>,
        workspace: &Workspace,
    ) -> Workspace {
        let config = &planned.config;
        let mut child_workspace = workspace.fork(child_control.shared_stop());
        child_workspace.services.deferred =
            workspace.services.deferred.as_ref().map(|deferred| Arc::new(deferred.only(|name| config.allows(name))));
        if planned.model != self.request.model {
            child_workspace.models.switch_to(planned.model.clone());
        }
        if let Some(window) = config.compact_window {
            let measured = crate::upkeep::window(&child_workspace.models.model);
            let caps = child_workspace.models.model.capabilities.get_or_insert_with(Default::default);
            caps.auto_compact_token_limit =
                Some(caps.auto_compact_token_limit.unwrap_or(measured * 9 / 10).min(window));
        }
        child_workspace.agent_name = chosen.name().into();
        if let Some(own) = config.observing() {
            child_workspace.observing = Some(own);
        }
        let mut scoped_hooks = config.hooks.clone();
        if let Some(stop) = scoped_hooks.0.remove(&crate::hooks::HookEvent::Stop) {
            scoped_hooks.0.entry(crate::hooks::HookEvent::SubagentStop).or_default().extend(stop);
        }
        child_workspace.hooks.extend(&scoped_hooks);
        child_workspace
    }

    fn settled(
        &self,
        chosen: &Chosen,
        planned: &Planned,
        child_workspace: &mut Workspace,
        run: &ThisRun,
    ) -> Result<(WorktreeLease, String), Outcome> {
        let config = &planned.config;
        let setup = crate::agent_runtime::prepare(config, chosen.name(), child_workspace)
            .map_err(|error| Outcome::error(format!("Agent setup failed: {error}")))?;
        let runtime = WorktreeLease::new(setup);
        let setup = &runtime.setup;
        child_workspace.cwd = setup.cwd.clone();
        if config.worktree {
            if let Some(context) = &mut child_workspace.prompt_context {
                context.cwd = setup.cwd.clone();
                context.git = setup.context.as_ref().and_then(|brief| brief.git.clone());
                context.instructions =
                    setup.context.as_ref().map(|brief| brief.instructions.clone()).unwrap_or_default();
            }
            child_workspace.rules = setup.context.as_ref().map(|brief| brief.rules.clone()).unwrap_or_default();
        }
        let mut system = self.child_system(chosen, config, child_workspace);
        if let Some(dir) = &setup.memory_dir {
            system.push_str(&format!("\n\n# Persistent agent memory\nYour memory directory is {}. Read and update MEMORY.md there as you learn durable facts. Keep its index below 200 lines and 25 KB; place details in other files. Do not store credentials.\n{}",dir.display(),setup.memory));
        }
        for skill in &config.skills {
            let Some(tool) = self.tool(run, names::SKILL) else {
                return Err(Outcome::error("The agent requests preloaded skills, but no Skill tool is available."));
            };
            let loaded = tool.run(&json!({"skill":skill}), child_workspace);
            if loaded.is_error {
                return Err(loaded);
            }
            system.push_str(&format!("\n\n# Preloaded skill\n{}", loaded.content));
        }
        Ok((runtime, system))
    }

    fn child_system(&self, chosen: &Chosen, config: &AgentConfig, child_workspace: &Workspace) -> String {
        let base = if let Some(mut context) = child_workspace.prompt_context.clone() {
            context.output_style = None;
            if config.omit_instructions {
                context.instructions.clear();
            }
            crate::prompt::system(&context)
        } else {
            self.request.system.clone()
        };
        if chosen.forked { self.request.system.clone() } else { subagent::system(&base, chosen.agent.as_ref()) }
    }

    fn announced(&self, chosen: &Chosen, input: &Value, task: &str, resume: Option<&Resume>) -> Lane {
        let kind = chosen.name().to_owned();
        let summary = input["description"].as_str().unwrap_or_default().to_owned();
        let index = match resume {
            Some(resume) => resume.index,
            None => {
                let index = self.control.next_agent();
                (self.emit)(EngineEvent::AgentSpawned {
                    index,
                    kind: kind.clone(),
                    summary: summary.clone(),
                    prompt: task.to_owned(),
                });
                index
            }
        };
        (self.emit)(EngineEvent::AgentUpdate { index, status: AgentStatus::Running { progress: 0.0 }, summary: None });
        Lane { index, kind, summary }
    }

    fn relayed(&self, index: usize, saying: &Mutex<String>, event: EngineEvent) {
        match event {
            EngineEvent::TextDelta(_)
            | EngineEvent::ThinkingDelta(_)
            | EngineEvent::ToolStarted(_)
            | EngineEvent::ToolFinished { .. }
            | EngineEvent::ToolResult { .. }
            | EngineEvent::Notice(_)
            | EngineEvent::Asked(_) => {
                match &event {
                    EngineEvent::TextDelta(text) => saying.locked().push_str(text),
                    EngineEvent::ToolResult { .. } => saying.locked().clear(),
                    _ => {}
                }
                let asked = matches!(event, EngineEvent::Asked(_)).then(|| event.clone());
                (self.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) });
                asked.into_iter().for_each(self.emit);
            }
            EngineEvent::Answered { .. } => (self.emit)(event),
            _ => {}
        }
    }

    fn child_tools(&self, config: &AgentConfig, planning: bool, run: &ThisRun) -> Vec<ToolSpec> {
        self.request
            .tools
            .iter()
            .filter(|spec| !top_level_only(&spec.name) && config.allows(&spec.name))
            .filter(|spec| !planning || !self.refused_while_planning(&spec.name, run))
            .cloned()
            .collect()
    }

    fn refused_while_planning(&self, name: &str, run: &ThisRun) -> bool {
        self.tool(run, name)
            .is_some_and(|tool| !crate::tools::changes_nothing(tool, &serde_json::Value::Null, self.workspace))
    }

    fn reported(&self, ran: Ran, lane: Lane, planned: &Planned, spawning: &Spawning<'_>) -> Outcome {
        let Lane { index, summary, .. } = lane;
        let report = report::last_said(&ran.history, ran.own);
        let report = match ran.cleanup {
            Ok(Some(path)) => format!("{report}\n\nChanges retained in worktree: {}", path.display()),
            Ok(None) => report,
            Err(error) => format!("{report}\n\nWorktree cleanup failed; no forced deletion attempted: {error}"),
        };
        match ran.outcome {
            Ok(_) => {
                let status = AgentStatus::Done { seconds: ran.seconds };
                (self.emit)(EngineEvent::AgentUpdate { index, status, summary: Some(summary) });
                let mut report =
                    if report.trim().is_empty() { "The sub-agent finished without a report.".into() } else { report };
                report.push_str(&report::how_it_ran(planned.unisolated, planned.unserved.as_deref(), &planned.model));
                if !planned.config.worktree {
                    let kept = Kept { input: spawning.input.clone(), index, history: ran.history };
                    let id = spawning.conversations.keep(kept);
                    report.push_str(&format!("\n\nagent_id: {id} (SendMessage continues it)"));
                }
                Outcome::ok(report)
            }
            Err(ending) => {
                let (status, outcome) = report::stopped(&ending, ran.unfinished, report);
                (self.emit)(EngineEvent::AgentUpdate { index, status, summary: None });
                outcome
            }
        }
    }
}

fn asked_turns(input: &Value) -> Result<usize, Outcome> {
    match input.get("max_turns") {
        Some(value) => match value.as_u64().filter(|n| *n > 0 && *n <= MAX_TURNS as u64) {
            Some(turns) => Ok(turns as usize),
            None => Err(Outcome::error("max_turns must be within 1..200")),
        },
        None => Ok(MAX_TURNS),
    }
}

fn task(input: &Value, resume: Option<&Resume>) -> Result<String, Outcome> {
    match resume {
        Some(resume) => Ok(resume.message.clone()),
        None => match input["prompt"].as_str().filter(|task| !task.trim().is_empty()) {
            Some(task) => Ok(task.to_owned()),
            None => Err(Outcome::error("Missing required parameter `prompt`.")),
        },
    }
}

fn first_history(resume: Option<Resume>, forked: bool, parent: &[Message], task: &str) -> Vec<Message> {
    match resume {
        Some(resume) => {
            let mut history = resume.history;
            history.push(Message::user(task));
            history
        }
        None if forked => builtin_agents::fork_history(parent, task),
        None => vec![Message::user(task)],
    }
}

fn started_then_run(
    child: &Agent<'_>,
    run: &ThisRun,
    lane: &Lane,
    task: &str,
    resumed: bool,
    history: &mut Vec<Message>,
) -> Result<Usage, Ending> {
    let start_hook = if resumed {
        Ok(Default::default())
    } else {
        let said = json!({"agent_type":lane.kind,"prompt":task});
        child.workspace.hooks.run(crate::hooks::HookEvent::SubagentStart, &lane.kind, said, child.workspace)
    };
    match start_hook {
        Ok(hook) if hook.blocked.is_none() => {
            if !hook.context.is_empty() {
                history.push(Message::user(hook.context));
            }
            super::current::within(lane.index, || child.run_in(run, history))
        }
        Ok(hook) => Err(Ending::Failed(ModelError::Provider(hook.blocked.unwrap_or_default()))),
        Err(error) => Err(Ending::Failed(ModelError::Provider(error))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turns_asked_for_stay_within_the_run_s_limit() {
        assert_eq!(asked_turns(&json!({})).ok(), Some(MAX_TURNS));
        assert_eq!(asked_turns(&json!({"max_turns": 3})).ok(), Some(3));
        for refused in [json!({"max_turns": 0}), json!({"max_turns": 201}), json!({"max_turns": "many"})] {
            assert!(asked_turns(&refused).is_err_and(|outcome| outcome.content.contains("1..200")), "{refused}");
        }
    }

    #[test]
    fn a_new_agent_needs_a_prompt_and_a_resumed_one_takes_the_message_sent() {
        assert_eq!(task(&json!({"prompt": "look"}), None).ok().as_deref(), Some("look"));
        assert!(task(&json!({"prompt": "  "}), None).is_err());
        let resume = Resume { index: 0, history: vec![Message::user("earlier")], message: "go on".into() };
        assert_eq!(task(&json!({}), Some(&resume)).ok().as_deref(), Some("go on"));
        let history = first_history(Some(resume), false, &[], "go on");
        assert_eq!(history.iter().map(Message::text).collect::<Vec<_>>(), ["earlier", "go on"]);
        assert_eq!(first_history(None, false, &[Message::user("parent")], "look").len(), 1);
    }
}
