use std::sync::Arc;
use std::time::Instant;

use super::ApiSource;
use super::converse::Prepared;
use crate::RunGate;
use crate::control::Control;
use crate::model::http::Http;
use crate::sessions::Sessions;
use crate::tools::{self, Workspace};
use domain::{EngineEvent, EngineProblem, FailureKind, PromptRequest, SessionCommand, SessionId};

pub(super) const SHARED_TOO_EARLY: &str = "The run's workspace was shared before its setup finished.";

pub(super) struct Job {
    pub(super) source: Arc<dyn ApiSource>,
    pub(super) sessions: Arc<dyn Sessions>,
    pub(super) mcp: Arc<dyn Http>,
    pub(super) lsp: Arc<dyn crate::lsp::LanguageServers>,
    pub(super) request: PromptRequest,
    pub(super) workspace: Arc<Workspace>,
    pub(super) control: Arc<Control>,
    pub(super) gate: RunGate,
    pub(super) recorder: Option<Arc<super::record::Recorder>>,
}

impl Job {
    pub(super) fn run(mut self) {
        let started = Instant::now();
        if self.request.command.is_some_and(|c| c != SessionCommand::Title) && self.request.resume.is_none() {
            return self.fail("There is no conversation to work on yet.", FailureKind::Other);
        }
        let (session, history) = match &self.request.resume {
            Some(id) => match self.sessions.load_for_checked(&self.request, id) {
                Ok(Some(history)) => (id.clone(), history),
                Err(error) => {
                    return self.fail(&format!("Could not restore the conversation: {error}"), FailureKind::Other);
                }
                Ok(None) if self.request.command.is_some_and(SessionCommand::is_chore) => (id.clone(), Vec::new()),
                Ok(None) => {
                    self.gate.emit(EngineEvent::Diagnosed(EngineProblem::SessionNotFound));
                    return self.fail("The conversation to continue was not found.", FailureKind::Other);
                }
            },
            None => (SessionId(uuid::Uuid::new_v4().to_string()), Vec::new()),
        };
        let session = if self.request.fork && self.request.command.is_none() {
            SessionId(uuid::Uuid::new_v4().to_string())
        } else {
            session
        };
        if let Some(command) = self.request.command.filter(|c| c.is_chore()) {
            return self.chore(command, &history, started);
        }
        if let Some(SessionCommand::Rewind(index)) = self.request.command {
            return self.rewind(&session, history, index, started);
        }
        if let Some(SessionCommand::Branch(index)) = self.request.command {
            return self.branch(&session, &history, index, started);
        }
        let recorder = super::record::Recorder::start(Arc::clone(&self.sessions), &self.request, &session);
        self.gate.observe(recorder.observer());
        self.recorder = Some(recorder);
        self.gate.emit(EngineEvent::SessionStarted(session.clone()));
        let brief = match crate::brief::Brief::load(&self.request) {
            Ok(brief) => brief,
            Err(error) => return self.fail(&error, FailureKind::Other),
        };
        let mcp_sources = brief.mcp();
        let Some(workspace) = Arc::get_mut(&mut self.workspace) else {
            return self.fail(SHARED_TOO_EARLY, FailureKind::Other);
        };
        workspace.rules = brief.rules;
        let mut tools = tools::standard();
        let specs = domain::lsp::servers(domain::lsp::defaults(), &brief.settings.lsp_servers);
        let lsp = crate::lsp::Lsp { servers: Arc::clone(&self.lsp), specs: Arc::new(specs) };
        if !lsp.available().is_empty() {
            workspace.services.lsp = Some(lsp);
            tools.push(Box::new(tools::Lsp));
        }
        let skills = brief.skills;
        if !skills.is_empty() {
            tools.push(Box::new(tools::skill::SkillTool { skills: skills.clone() }));
        }
        let agents = brief.agents;
        tools.push(Box::new(tools::subagent::AgentTool { agents: agents.clone() }));
        tools.push(Box::new(tools::send_message::SendMessageTool));
        tools.extend(tools::task::tools());
        let servers = crate::mcp::connect_stopped(&self.request, &mcp_sources, &self.mcp, self.control.stop_flag());
        tools.extend(crate::mcp::tools_of(&servers, self.control.stop_flag()));
        let home = crate::brief::home(&self.request);
        tools::script_tool::join(&mut tools, tools::script_tool::load(home.as_deref(), &self.request.cwd));
        match crate::mcp::extras::expand_prompt(&servers, &self.request.prompt, self.control.stop_flag()) {
            Some(Ok(expanded)) => self.request.prompt = expanded,
            Some(Err(error)) => return self.fail(&error, FailureKind::Other),
            None => {}
        }
        let settings = brief.settings;
        let selected = if let Some(name) = &settings.agent {
            match (tools::subagent::AgentTool { agents: agents.clone() })
                .chosen(&serde_json::json!({"subagent_type":name}))
            {
                Ok(Some(agent)) => Some(agent.clone()),
                Ok(None) => None,
                Err(error) => return self.fail(&error, FailureKind::Other),
            }
        } else {
            None
        };
        let mut agent_config = selected.as_ref().map(|agent| agent.config.clone()).unwrap_or_default();
        if !settings.auto_memory {
            agent_config.memory = None;
        }
        self.control = self.control.child_configured(agent_config.permission, agent_config.dont_ask);
        let Some(workspace) = Arc::get_mut(&mut self.workspace) else {
            return self.fail(SHARED_TOO_EARLY, FailureKind::Other);
        };
        workspace.stop = self.control.shared_stop();
        workspace.auto_memory = settings.auto_memory;
        workspace.policy.scripts = settings.scripts;
        workspace.policy.sandbox = settings.features.on(domain::features::SANDBOX).then(|| settings.sandbox.clone());
        workspace.policy.redact = settings
            .features
            .on(domain::features::REDACT_SECRETS)
            .then(|| Arc::new(tools::env_secrets(&workspace.cwd)));
        workspace.policy.snapshots = settings.features.on(domain::features::DESTRUCTIVE_SNAPSHOT);
        workspace.policy.dependency_guard = settings.features.on(domain::features::DEPENDENCY_GUARD);
        workspace.policy.escalation = settings.features.on(domain::features::ESCALATION);
        workspace.policy.auto_review = settings.features.on(domain::features::AUTO_REVIEW);
        workspace.policy.affected_tests = settings.features.on(domain::features::AFFECTED_TESTS);
        workspace.policy.session_claims = settings.features.on(domain::features::SESSION_CLAIMS);
        workspace.policy.web = domain::web_reading::WebReading::from(&settings.features);
        workspace.models.escalation.clone_from(&settings.escalation);
        if !settings.features.on(domain::features::BUDGET) {
            self.request.budget = None;
        }
        workspace.services.files = Some(Arc::default());
        if self.request.safety_check {
            workspace.policy.safety = Some(Arc::new(crate::guard::Safety::default()));
        }
        if let Some(name) = &settings.advisor {
            let config = tools::subagent_config::AgentConfig { model: Some(name.clone()), ..Default::default() };
            match config.model(&self.request.model.id, self.request.model.provider) {
                Ok(model) => workspace.models.advisor = Some(model),
                Err(error) => return self.fail(&format!("advisorModel: {error}"), FailureKind::Other),
            }
        }
        tools.retain(|tool| tool.offered(workspace));
        let project_criteria = settings
            .project_criteria
            .clone()
            .filter(|_| settings.features.on(domain::features::PROJECT_CRITERIA))
            .map(domain::Criteria::project);
        if let Some(criteria) = self
            .request
            .verification
            .clone()
            .or(project_criteria)
            .filter(|_| self.request.permission != domain::PermissionMode::Plan)
        {
            let config = tools::subagent_config::AgentConfig { model: settings.judge.clone(), ..Default::default() };
            match config.model(&self.request.model.id, self.request.model.provider) {
                Ok(model) => {
                    let allow = criteria.judge_allow(&settings.judge_allow);
                    workspace.verification = Some(tools::Verification { criteria, model, allow })
                }
                Err(error) => return self.fail(&format!("judgeModel: {error}"), FailureKind::Other),
            }
        }
        workspace.agent_name = selected.as_ref().map_or("main", |a| a.name.as_str()).into();
        workspace.session_id = session.0.clone();
        workspace.hooks = settings.hooks.clone();
        workspace.anchored_memories = self
            .request
            .memory
            .as_ref()
            .filter(|brief| brief.anchors && settings.features.on(domain::features::MEMORY_ANCHORS))
            .map(|brief| brief.anchored.clone())
            .unwrap_or_default();
        workspace.blast_radius = settings.features.on(domain::features::BLAST_RADIUS);
        workspace.policy.permissions = settings.permissions.clone();
        for (kind, rules) in [
            (domain::permission_rules::RuleKind::Allow, &self.request.allowed_tools),
            (domain::permission_rules::RuleKind::Deny, &self.request.disallowed_tools),
        ] {
            for rule in rules {
                if let Err(error) = workspace.policy.permissions.add(kind, rule) {
                    return self.fail(&format!("--allowedTools/--disallowedTools: {error}"), FailureKind::Other);
                }
            }
        }
        workspace.home = crate::brief::home(&self.request);
        workspace.models.fallbacks = match &self.request.fallback_model {
            Some(model) => vec![model.clone()],
            None => settings.fallbacks.clone(),
        };
        let small = super::small_model::pick(
            self.request.model.provider,
            &self.request.model.id,
            settings.small_model.as_deref(),
            &self.source.catalog(&self.request),
        );
        if let Some(why) = &small.ignored {
            tracing::warn!("{why}");
        }
        workspace.models.small_model = small.model;
        workspace.services.network = Some(tools::Network(Arc::clone(&self.mcp)));
        workspace.added_dirs = crate::brief::added_dirs(&self.request, &settings.added_dirs, workspace.home.as_deref());
        workspace.hooks.extend(&agent_config.hooks);
        let previous = match self.sessions.load_worktree(&self.request, &session) {
            Ok(state) => state,
            Err(error) => return self.fail(&error, FailureKind::Other),
        };
        let resumed_worktree = previous.is_some();
        let name = selected.as_ref().map_or("main", |agent| agent.name.as_str());
        let setup = match if let Some(state) = previous {
            crate::agent_runtime::resume(&agent_config, name, &state, workspace)
        } else {
            crate::agent_runtime::prepare(&agent_config, name, workspace)
        } {
            Ok(setup) => setup,
            Err(error) => return self.fail(&error, FailureKind::Other),
        };
        workspace.cwd = setup.cwd.clone();
        workspace.worktree = setup.worktree_state();
        if let Some(context) = &setup.context {
            workspace.rules = context.rules.clone();
        }
        let cleanup_workspace = workspace.fork(Arc::default());
        let mut runtime_guard = crate::agent_runtime::Guard::new(setup);
        if resumed_worktree {
            runtime_guard.retain();
        }
        if let Some(state) = runtime_guard.setup.worktree_state() {
            let stored = self
                .sessions
                .save_for(&self.request, &session, &history)
                .and_then(|_| self.sessions.save_worktree(&self.request, &session, &state));
            if let Err(error) = stored {
                let retained = runtime_guard.finish();
                let error = match retained {
                    Ok(Some(path)) => format!("{error}; worktree retained at {}", path.display()),
                    Err(cleanup) => format!("{error}; worktree cleanup failed: {cleanup}"),
                    _ => error,
                };
                return self.fail(&error, FailureKind::Other);
            }
            runtime_guard.retain();
        }
        self.request.effort = agent_config.effort.unwrap_or(self.request.effort);
        match agent_config.model(&self.request.model.id, self.request.model.provider) {
            Ok(model) if model != self.request.model.id => {
                self.request.model.id = model.clone();
                workspace.models.model.id = model;
                workspace.models.model.capabilities = None;
                workspace.models.model.window = None;
            }
            Err(error) => return self.fail(&error, FailureKind::Other),
            _ => {}
        }
        if let Some(schema) = settings.schema {
            match tools::structured::Structured::new(schema) {
                Ok(tool) => tools.push(Box::new(tool)),
                Err(error) => return self.fail(&error, FailureKind::Other),
            }
        }
        let prepared = Prepared {
            started,
            session,
            history,
            tools,
            skills,
            agents,
            selected,
            agent_config,
            style: settings.style,
            git: brief.git,
            instructions: brief.instructions,
        };
        self.converse(prepared, &mut runtime_guard, &cleanup_workspace);
    }
}
