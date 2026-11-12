use domain::{AgentStatus, EngineEvent};
use serde_json::{Value, json};

use super::conversations::{Conversations, Kept, Resume};
use super::{Agent, Ending, MAX_TURNS};
use crate::control::Control;
use crate::model::{ModelError, TurnRequest};
use crate::tools::builtin_agents;
use crate::tools::send_message::SEND_MESSAGE_TOOL;
use crate::tools::subagent::{self, AGENT_TOOL, AgentTool};
use crate::tools::{Outcome, Workspace};
use crate::transcript::{Message, Role};

const TOP_LEVEL_ONLY: [&str; 6] = [
    AGENT_TOOL,
    SEND_MESSAGE_TOOL,
    crate::tools::plan_mode::EXIT_PLAN_TOOL,
    crate::tools::plan_mode::ENTER_PLAN_TOOL,
    crate::tools::worktree::ENTER_WORKTREE,
    crate::tools::worktree::EXIT_WORKTREE,
];

impl Agent<'_> {
    pub(super) fn sub_agent_with(
        &self,
        input: &Value,
        control: &Control,
        workspace: &Workspace,
        parent: &[Message],
        conversations: &Conversations,
        resume: Option<Resume>,
    ) -> Outcome {
        if self.depth > 0 {
            return Outcome::error("A sub-agent cannot start other sub-agents; do the work yourself.");
        }
        let chosen = match (AgentTool { agents: self.agents.to_vec() }).chosen(input) {
            Ok(chosen) => chosen.cloned(),
            Err(problem) => return Outcome::error(problem),
        };
        let forked = chosen.as_ref().is_some_and(|agent| {
            agent.name == builtin_agents::FORK && agent.instructions == builtin_agents::fork_preamble()
        });
        if forked && parent.is_empty() && resume.is_none() {
            return Outcome::error("A fork needs the conversation it forks from; none was given.");
        }
        let mut config = chosen.as_ref().map(|a| a.config.clone()).unwrap_or_default();
        if !workspace.auto_memory {
            config.memory = None;
        }
        if let Some(model) = input.get("model").and_then(serde_json::Value::as_str).filter(|m| !m.is_empty()) {
            let asked =
                crate::tools::subagent_config::AgentConfig { model: Some(model.to_owned()), ..Default::default() };
            if asked.model(&self.request.model, self.workspace.model.provider).is_ok() {
                config.model = asked.model;
            }
        }
        let explores = chosen.as_ref().is_some_and(|agent| agent.name == "Explore");
        if config.model.is_none() && explores {
            let small = crate::tools::subagent_config::AgentConfig {
                model: workspace.small_model.clone(),
                ..Default::default()
            };
            if small.model.is_some() && small.model(&self.request.model, self.workspace.model.provider).is_ok() {
                config.model = small.model;
            }
        }
        if input.get("isolation").and_then(serde_json::Value::as_str) == Some("worktree") {
            config.worktree = true;
        }
        let model = match config.model(&self.request.model, self.workspace.model.provider) {
            Ok(model) => model,
            Err(error) => return Outcome::error(error),
        };
        let turn_limit = match input.get("max_turns") {
            Some(value) => match value.as_u64().filter(|n| *n > 0 && *n <= MAX_TURNS as u64) {
                Some(turns) => turns as usize,
                None => return Outcome::error("max_turns must be within 1..200"),
            },
            None => MAX_TURNS,
        }
        .min(config.max_turns.unwrap_or(MAX_TURNS));
        let child_control = control.child_configured(config.permission, config.dont_ask);
        let mut child_workspace = workspace.fork(child_control.shared_stop());
        child_workspace.deferred =
            workspace.deferred.as_ref().map(|deferred| std::sync::Arc::new(deferred.only(|name| config.allows(name))));
        if model != self.request.model {
            child_workspace.model.id = model.clone();
            child_workspace.model.window = None;
            child_workspace.model.capabilities = None;
        }
        if let Some(window) = config.compact_window {
            let measured = crate::upkeep::window(&child_workspace.model);
            let caps = child_workspace.model.capabilities.get_or_insert_with(Default::default);
            caps.auto_compact_token_limit =
                Some(caps.auto_compact_token_limit.unwrap_or(measured * 9 / 10).min(window));
        }
        child_workspace.agent_name = chosen.as_ref().map_or("general-purpose", |a| a.name.as_str()).into();
        let mut scoped_hooks = config.hooks.clone();
        if let Some(stop) = scoped_hooks.0.remove("Stop") {
            scoped_hooks.0.entry("SubagentStop".into()).or_default().extend(stop);
        }
        child_workspace.hooks.extend(&scoped_hooks);
        let setup = match crate::agent_runtime::prepare(
            &config,
            chosen.as_ref().map_or("general-purpose", |a| a.name.as_str()),
            &child_workspace,
        ) {
            Ok(setup) => setup,
            Err(error) => return Outcome::error(format!("Agent setup failed: {error}")),
        };
        let mut runtime_guard = crate::agent_runtime::Guard::new(setup);
        let setup = &runtime_guard.setup;
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
        let base = if let Some(mut context) = child_workspace.prompt_context.clone() {
            context.output_style = None;
            if config.omit_instructions {
                context.instructions.clear();
            }
            crate::prompt::system(&context)
        } else {
            self.request.system.clone()
        };
        let mut system = if forked { self.request.system.clone() } else { subagent::system(&base, chosen.as_ref()) };
        if let Some(dir) = &setup.memory_dir {
            system.push_str(&format!("\n\n# Persistent agent memory\nYour memory directory is {}. Read and update MEMORY.md there as you learn durable facts. Keep its index below 200 lines and 25 KB; place details in other files. Do not store credentials.\n{}",dir.display(),setup.memory));
        }
        for skill in &config.skills {
            let Some(tool) = self.tools.iter().find(|tool| tool.spec().name == "Skill") else {
                return Outcome::error("The agent requests preloaded skills, but no Skill tool is available.");
            };
            let loaded = tool.run(&json!({"skill":skill}), &child_workspace);
            if loaded.is_error {
                return loaded;
            }
            system.push_str(&format!("\n\n# Preloaded skill\n{}", loaded.content));
        }
        let task = match &resume {
            Some(resume) => resume.message.clone(),
            None => match input["prompt"].as_str().filter(|task| !task.trim().is_empty()) {
                Some(task) => task.to_owned(),
                None => return Outcome::error("Missing required parameter `prompt`."),
            },
        };
        let task = task.as_str();
        let kind = chosen.as_ref().map_or(subagent::GENERAL, |agent| agent.name.as_str()).to_owned();
        let summary = input["description"].as_str().unwrap_or_default().to_owned();
        let index = match &resume {
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
        let relay = |event: EngineEvent| match event {
            EngineEvent::TextDelta(_)
            | EngineEvent::ThinkingDelta(_)
            | EngineEvent::ToolStarted(_)
            | EngineEvent::ToolFinished { .. }
            | EngineEvent::ToolResult { .. } => {
                (self.emit)(EngineEvent::AgentEvent { index, event: Box::new(event) });
            }
            EngineEvent::Asked(_) => (self.emit)(event),
            _ => {}
        };
        let started = std::time::Instant::now();
        let child = Agent {
            api: self.api,
            tools: self.tools,
            workspace: &child_workspace,
            control: &child_control,
            emit: &relay,
            request: TurnRequest {
                model,
                system,
                tools: self
                    .request
                    .tools
                    .iter()
                    .filter(|spec| !TOP_LEVEL_ONLY.contains(&spec.name.as_str()) && config.allows(&spec.name))
                    .cloned()
                    .collect(),
                effort: config.effort.unwrap_or(self.request.effort),
                ..self.request.clone()
            },
            agents: &[],
            depth: self.depth + 1,
            turn_limit,
        };
        let resumed = resume.is_some();
        let mut history = match resume {
            Some(resume) => {
                let mut history = resume.history;
                history.push(Message::user(task));
                history
            }
            None if forked => builtin_agents::fork_history(parent, task),
            None => vec![Message::user(task)],
        };
        let own = history.len();
        let start_hook = if resumed {
            Ok(Default::default())
        } else {
            child_workspace.hooks.run(
                "SubagentStart",
                &kind,
                json!({"agent_type":kind,"prompt":task}),
                &child_workspace,
            )
        };
        let outcome = match start_hook {
            Ok(hook) if hook.blocked.is_none() => {
                if !hook.context.is_empty() {
                    history.push(Message::user(hook.context));
                }
                super::current::within(index, || child.run(&mut history))
            }
            Ok(hook) => Err(Ending::Failed(ModelError::Provider(hook.blocked.unwrap_or_default()))),
            Err(error) => Err(Ending::Failed(ModelError::Provider(error))),
        };
        let cleanup = runtime_guard.finish();
        let report = history
            .get(if history.len() >= own { own } else { 0 }..)
            .unwrap_or_default()
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
            .map(Message::text)
            .unwrap_or_default();
        let report = match cleanup {
            Ok(Some(path)) => format!("{report}\n\nChanges retained in worktree: {}", path.display()),
            Ok(None) => report,
            Err(error) => format!("{report}\n\nWorktree cleanup failed; no forced deletion attempted: {error}"),
        };
        let seconds = u32::try_from(started.elapsed().as_secs()).unwrap_or(u32::MAX);
        match outcome {
            Ok(_) => {
                let status = AgentStatus::Done { seconds };
                (self.emit)(EngineEvent::AgentUpdate { index, status, summary: Some(summary) });
                let mut report =
                    if report.trim().is_empty() { "The sub-agent finished without a report.".into() } else { report };
                if !config.worktree {
                    let id = conversations.keep(Kept { input: input.clone(), index, history });
                    report.push_str(&format!("\n\nagent_id: {id} (SendMessage continues it)"));
                }
                Outcome::ok(report)
            }
            Err(ending) => {
                (self.emit)(EngineEvent::AgentUpdate { index, status: AgentStatus::Failed, summary: None });
                Outcome::error(format!("The sub-agent stopped: {ending:?}. What it said last: {report}"))
            }
        }
    }
}
