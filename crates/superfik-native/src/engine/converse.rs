//! One run, second half: the system prompt, the session commands that need
//! it (measuring, compacting), the session hooks, the agent loop itself,
//! and saving the conversation it leaves.

use std::sync::Arc;
use std::time::Instant;

use superfik_core::{EngineEvent, FailureKind, SessionCommand, SessionId};

use super::MAX_OUTPUT;
use super::date::today;
use super::job::Job;
use crate::agent::{Agent, Ending};
use crate::agent_runtime::Guard;
use crate::model::TurnRequest;
use crate::prompt::{self, Environment};
use crate::tools::subagent::FoundAgent;
use crate::tools::subagent_config::AgentConfig;
use crate::tools::{self, Tool, Workspace};
use crate::transcript::Message;
use crate::upkeep;

pub(super) struct Prepared {
    pub(super) started: Instant,
    pub(super) session: SessionId,
    pub(super) history: Vec<Message>,
    pub(super) tools: Vec<Box<dyn Tool>>,
    pub(super) skills: Vec<tools::skill::FoundSkill>,
    pub(super) agents: Vec<FoundAgent>,
    pub(super) selected: Option<FoundAgent>,
    pub(super) agent_config: AgentConfig,
    pub(super) style: Option<prompt::settings::Style>,
    pub(super) git: Option<prompt::GitSnapshot>,
    pub(super) instructions: Vec<prompt::Instructions>,
    pub(super) images: Vec<crate::image::Image>,
}

impl Job {
    pub(super) fn converse(
        &mut self,
        prepared: Prepared,
        runtime_guard: &mut Guard<'_>,
        cleanup_workspace: &Workspace,
    ) {
        let Prepared {
            started,
            session,
            mut history,
            tools,
            skills,
            agents,
            selected,
            agent_config,
            style,
            git,
            instructions,
            images,
        } = prepared;
        let environment = Environment {
            cwd: self.workspace.cwd.clone(),
            platform: prompt::platform(),
            date: today(),
            model: self.request.model.label.clone(),
            git: runtime_guard.setup.context.as_ref().and_then(|b| b.git.clone()).or(git),
            instructions: runtime_guard.setup.context.as_ref().map(|b| b.instructions.clone()).unwrap_or(instructions),
            tools: tools.iter().map(|tool| tool.spec().name).collect(),
            skills: tools::skill::listing(&skills),
            output_style: style.or_else(|| {
                let picked = self.request.output_style.as_deref()?;
                prompt::styles::builtin(picked).map(|style| style.style())
            }),
        };
        Arc::get_mut(&mut self.workspace).expect("unshared context").prompt_context = Some(environment.clone());
        let mut main_system = prompt::system(&environment);
        if let Some(agent) = &selected {
            main_system.push_str(&format!("\n\n# Agent instructions ({})\n{}", agent.name, agent.instructions));
        }
        if let Some(dir) = &runtime_guard.setup.memory_dir {
            main_system.push_str(&format!("\n\n# Persistent agent memory\nRead and update MEMORY.md in {}. Keep its index under 200 lines and 25 KB.\n{}",dir.display(),runtime_guard.setup.memory));
        }
        for skill in &agent_config.skills {
            let Some(tool) = tools.iter().find(|tool| tool.spec().name == "Skill") else {
                return self.fail("Configured agent requires the Skill tool", FailureKind::Other);
            };
            let loaded = tool.run(&serde_json::json!({"skill":skill}), &self.workspace);
            if loaded.is_error {
                return self.fail(&loaded.content, FailureKind::Other);
            }
            main_system.push_str(&format!("\n\n# Preloaded skill\n{}", loaded.content));
        }
        let template = TurnRequest {
            model: self.request.model.id.clone(),
            system: main_system,
            messages: Vec::new(),
            tools: Agent::specs(&tools).into_iter().filter(|spec| agent_config.allows(&spec.name)).collect(),
            effort: self.request.effort,
            max_tokens: MAX_OUTPUT,
        };
        if self.request.command == Some(SessionCommand::MeasureContext) {
            let usage = upkeep::measure(&self.request.model, &template.system, &template.tools, &history);
            self.gate.emit(EngineEvent::ContextMeasured(usage));
            return self.completed(started, Some(0.0));
        }
        let api = match self.source.api(&self.request) {
            Ok(api) => api,
            Err(error) => return self.model_failed(error),
        };
        let ledger = Arc::new(crate::meter::Ledger::default());
        let api = crate::meter::Metered {
            api,
            ledger: ledger.clone(),
            provider: self.request.model.provider,
            gate: self.gate.clone(),
            model: self.request.model.clone(),
            guard: self.request.credentials.model_guard.clone(),
        };
        if self.request.command == Some(SessionCommand::Compact) {
            return match upkeep::compact_hooked(&api, &template, &mut history, &self.workspace, "manual") {
                Ok(compaction) => {
                    if let Err(error) = self.sessions.save_for(&self.request, &session, &history) {
                        return self
                            .fail(&format!("Could not save the compacted conversation: {error}"), FailureKind::Other);
                    }
                    self.gate.emit(EngineEvent::Compacted(compaction));
                    self.completed(started, ledger.cost())
                }
                Err(error) => self.model_failed(error),
            };
        }
        let emit = |event: EngineEvent| {
            self.gate.emit(event);
        };
        let agent = Agent {
            api: &api,
            tools: &tools,
            workspace: &self.workspace,
            control: &self.control,
            emit: &emit,
            request: template,
            agents: &agents,
            depth: 0,
            turn_limit: agent_config.max_turns.unwrap_or(crate::agent::MAX_TURNS),
        };
        let initial = if history.is_empty() { agent_config.initial_prompt.as_deref().unwrap_or_default() } else { "" };
        let initial = match tools::skill::expand_initial(initial, &skills, &self.workspace) {
            Ok(initial) => initial,
            Err(error) => return self.fail(&error, FailureKind::Other),
        };
        let mut user = Message::user(format!(
            "{}{}{}",
            initial,
            if initial.is_empty() { "" } else { "\n\n" },
            prompt::user_message(&self.request)
        ));
        let source = if self.request.resume.is_some() { "resume" } else { "startup" };
        for (event, subject, payload) in [
            (
                "SessionStart",
                source,
                serde_json::json!({"source":source,"session_id":session,"model":self.request.model.id}),
            ),
            ("UserPromptSubmit", "", serde_json::json!({"prompt":user.text(),"session_id":session})),
        ] {
            match self.workspace.hooks.run_with_model(
                event,
                subject,
                payload,
                &self.workspace,
                Some((&api, &agent.request)),
            ) {
                Ok(hook) if hook.blocked.is_none() => {
                    if !hook.context.is_empty() {
                        user.blocks.push(crate::transcript::Block::Text(hook.context));
                    }
                }
                Ok(hook) => return self.fail(&hook.blocked.unwrap_or_default(), FailureKind::Other),
                Err(error) => return self.fail(&error, FailureKind::Other),
            }
        }
        user.blocks.extend(images.into_iter().map(crate::transcript::Block::Image));
        history.push(user);
        let outcome = agent.run(&mut history);
        let _ =
            self.workspace.hooks.run("SessionEnd", "", serde_json::json!({"session_id":session}), cleanup_workspace);
        match runtime_guard.finish() {
            Ok(Some(path)) => {
                self.gate.emit(EngineEvent::TextDelta(format!("\nChanges retained in worktree: {}", path.display())));
            }
            Err(error) => {
                self.gate.emit(EngineEvent::TextDelta(format!("\nWorktree cleanup failed; files retained: {error}")));
            }
            _ => {}
        }
        if let Err(error) = self.sessions.save_for(&self.request, &session, &history) {
            return self.fail(&format!("Could not save the conversation: {error}"), FailureKind::Other);
        }
        match outcome {
            Ok(_usage) => self.completed(started, ledger.cost()),
            Err(Ending::Cancelled) => {
                self.gate.emit(EngineEvent::Cancelled);
            }
            Err(Ending::Failed(error)) => self.model_failed(error),
            Err(Ending::Refused) => self.fail("The model declined to continue with this request.", FailureKind::Other),
            Err(Ending::TooManyTurns) => {
                self.fail("The agent stopped after too many steps without finishing.", FailureKind::Other);
            }
        }
    }
}
