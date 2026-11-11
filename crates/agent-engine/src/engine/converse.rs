use std::sync::Arc;
use std::time::Instant;

use domain::{EngineEvent, FailureKind, PromptRequest, SessionCommand, SessionId};

use super::MAX_OUTPUT;
use super::date::today;
use super::job::Job;
use super::moved;
use crate::agent::{Agent, Ending};
use crate::agent_runtime::Guard;
use crate::model::TurnRequest;
use crate::prompt::{self, Environment};
use crate::tools::subagent::FoundAgent;
use crate::tools::subagent_config::AgentConfig;
use crate::tools::{self, Tool, Workspace};
use crate::transcript::{Message, Role};
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
}

impl Job {
    pub(super) fn converse(&mut self, prepared: Prepared, runtime_guard: &mut Guard, cleanup_workspace: &Workspace) {
        let Prepared {
            started,
            session,
            mut history,
            mut tools,
            skills,
            agents,
            selected,
            agent_config,
            style,
            git,
            instructions,
        } = prepared;
        let environment = Environment {
            cwd: self.workspace.cwd.clone(),
            added_dirs: self.workspace.added_dirs.clone(),
            platform: prompt::platform(),
            date: today(),
            model: self.request.model.label.clone(),
            git: runtime_guard.setup.context.as_ref().and_then(|b| b.git.clone()).or(git),
            instructions: runtime_guard.setup.context.as_ref().map(|b| b.instructions.clone()).unwrap_or(instructions),
            tools: tools.iter().map(|tool| tool.spec().name).collect(),
            skills: tools::skill::listing(&skills),
            output_style: prompt::styles::picked(self.request.output_style.as_deref(), style),
        };
        Arc::get_mut(&mut self.workspace).expect("unshared context").prompt_context = Some(environment.clone());
        let mut main_system = prompt::system(&environment);
        let mut extra = String::new();
        if let Some(agent) = &selected {
            extra.push_str(&format!("\n\n# Agent instructions ({})\n{}", agent.name, agent.instructions));
        }
        if let Some(appended) = self.request.append_system.as_deref().filter(|text| !text.trim().is_empty()) {
            extra.push_str(&format!("\n\n{}", appended.trim()));
        }
        if let Some(dir) = &runtime_guard.setup.memory_dir {
            extra.push_str(&format!("\n\n# Persistent agent memory\nRead and update MEMORY.md in {}. Keep its index under 200 lines and 25 KB.\n{}",dir.display(),runtime_guard.setup.memory));
        }
        for skill in &agent_config.skills {
            let Some(tool) = tools.iter().find(|tool| tool.spec().name == "Skill") else {
                return self.fail("Configured agent requires the Skill tool", FailureKind::Other);
            };
            let loaded = tool.run(&serde_json::json!({"skill":skill}), &self.workspace);
            if loaded.is_error {
                return self.fail(&loaded.content, FailureKind::Other);
            }
            extra.push_str(&format!("\n\n# Preloaded skill\n{}", loaded.content));
        }
        let offered = Agent::specs(&tools).into_iter().filter(|spec| agent_config.allows(&spec.name)).collect();
        let (offered, deferred) = tools::tool_search::defer(offered);
        if let Some(deferred) = deferred {
            tools.push(Box::new(tools::tool_search::ToolSearch));
            Arc::get_mut(&mut self.workspace).expect("unshared context").deferred = Some(Arc::new(deferred));
        }
        main_system.push_str(&extra);
        let template = TurnRequest {
            model: self.request.model.id.clone(),
            system: main_system,
            messages: Vec::new(),
            tools: offered,
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
        };
        if self.request.command == Some(SessionCommand::Compact) {
            let focus = Some(self.request.prompt.as_str()).filter(|focus| !focus.trim().is_empty());
            return match upkeep::compact_hooked_on(&api, &template, &mut history, &self.workspace, "manual", focus) {
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
        let mut turn_limit = self
            .request
            .max_turns
            .map(|turns| turns as usize)
            .or(agent_config.max_turns)
            .unwrap_or(crate::agent::MAX_TURNS);
        let mut template = template;
        let initial = if history.is_empty() { agent_config.initial_prompt.as_deref().unwrap_or_default() } else { "" };
        let initial = match tools::skill::expand_initial(initial, &skills, &self.request.cwd) {
            Ok(initial) => initial,
            Err(error) => return self.fail(&error, FailureKind::Other),
        };
        let typed = match tools::skill::expand_command(&self.request.prompt, &skills, &self.request.cwd) {
            Some(Ok(prompt)) => prompt::user_message(&PromptRequest { prompt, ..self.request.clone() }),
            Some(Err(error)) => return self.fail(&error, FailureKind::Other),
            None => prompt::user_message(&self.request),
        };
        let mut user = Message::user(format!("{}{}{}", initial, if initial.is_empty() { "" } else { "\n\n" }, typed));
        user.blocks.splice(0..0, self.request.images.iter().cloned().map(crate::transcript::Block::Image));
        let source = if self.request.resume.is_some() { "resume" } else { "startup" };
        for (event, subject, payload) in [
            (
                "SessionStart",
                source,
                serde_json::json!({"source":source,"session_id":session,"model":self.request.model.id}),
            ),
            ("UserPromptSubmit", "", serde_json::json!({"prompt":user.text(),"session_id":session})),
        ] {
            match self.workspace.hooks.run_with_model(event, subject, payload, &self.workspace, Some((&api, &template)))
            {
                Ok(hook) if hook.blocked.is_none() => {
                    if !hook.context.is_empty() {
                        user.blocks.push(crate::transcript::Block::Text(hook.context));
                    }
                }
                Ok(hook) => return self.fail(&hook.blocked.unwrap_or_default(), FailureKind::Other),
                Err(error) => return self.fail(&error, FailureKind::Other),
            }
        }
        let checkpoint = self.open_checkpoint(&session, &history);
        history.push(user);
        let outcome = loop {
            let agent = Agent {
                api: &api,
                tools: &tools,
                workspace: &self.workspace,
                control: &self.control,
                emit: &emit,
                request: template.clone(),
                agents: &agents,
                depth: 0,
                turn_limit,
            };
            let before = history.len();
            let outcome = agent.run(&mut history);
            drop(agent);
            let used = history[before.min(history.len())..].iter().filter(|m| m.role == Role::Assistant).count();
            turn_limit = turn_limit.saturating_sub(used);
            let Some(relocation) = self.control.take_move() else { break outcome };
            let mover =
                moved::Move { request: &self.request, sessions: &*self.sessions, session: &session, extra: &extra };
            match mover.to(relocation, Arc::get_mut(&mut self.workspace), runtime_guard) {
                Ok(moved) => {
                    template.system = moved.system;
                    emit(EngineEvent::Moved(moved.cwd));
                    if let Some(kept) = moved.kept {
                        emit(EngineEvent::TextDelta(format!("\n{kept}")));
                    }
                }
                Err(error) => break Err(Ending::Failed(crate::model::ModelError::Provider(error))),
            }
            match outcome {
                Err(Ending::Moved) if turn_limit == 0 => break Err(Ending::TooManyTurns),
                Err(Ending::Moved) => {}
                other => break other,
            }
        };
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
        self.keep_checkpoint(&session, checkpoint);
        match outcome {
            Ok(_usage) => self.completed(started, ledger.cost()),
            Err(Ending::Cancelled) => {
                self.gate.emit(EngineEvent::Cancelled);
            }
            Err(Ending::Failed(error)) => self.model_failed(error),
            Err(Ending::Refused) => self.fail("The model declined to continue with this request.", FailureKind::Other),
            Err(Ending::Moved) => self.completed(started, ledger.cost()),
            Err(Ending::TooManyTurns) => {
                self.fail("The agent stopped after too many steps without finishing.", FailureKind::Other);
            }
        }
    }
}
