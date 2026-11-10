//! Executes one authorized command. Synchronous and transport-free, so a
//! local backend and a future network server share it.

use std::sync::Arc;

use superfik_core::{Author, EngineEvent, PluginBrief, Principal, PromptRequest, Provenance, RunId};
use superfik_engine::EventSink;
use superfik_protocol::{ClientCommand, Envelope, PROTOCOL_VERSION, ServerEvent};

use crate::memory::mcp::McpServer;
use crate::memory::{self, Memories};
use crate::outbox::Outbox;
use crate::plugins;
use crate::runs::{Reach, RunTable};
use crate::services::LocalServices;
use crate::workspace;

#[derive(Debug)]
pub struct CommandHandler {
    services: LocalServices,
    runs: Arc<RunTable>,
    outbox: Outbox,
    memory_tool: Option<McpServer>,
}

impl CommandHandler {
    pub fn new(services: LocalServices, outbox: Outbox) -> Self {
        let memory_tool = services.memory_tool.then(|| McpServer::start(services.memory.clone())).flatten();
        let runs = Arc::new(RunTable::default());
        Self { services, runs, outbox, memory_tool }
    }

    pub fn handle(&self, envelope: Envelope<ClientCommand>) {
        let Envelope { principal, payload: command } = envelope;
        let outbox = &self.outbox.answering(&principal);
        if let Err(error) = self.services.authorizer.authorize(&principal, &command) {
            tracing::warn!(%error, "command rejected");
            outbox.send(ServerEvent::error(error.to_string()));
            return;
        }
        tracing::debug!(command = command.name(), "handling command");
        match command {
            ClientCommand::SubmitPrompt { run, request } => {
                self.submit(&principal, run, &crate::sent::as_sent(&request));
            }
            ClientCommand::CancelRun { run } => {
                Self::reached(&principal, outbox, "cancel_run", run, self.runs.cancel(run, &principal));
            }
            ClientCommand::AnswerAsk { run, ask, decision } => {
                let reach = self.runs.control(run, &principal, |handle| handle.answer(&ask, &decision));
                Self::reached(&principal, outbox, "answer_ask", run, reach);
            }
            ClientCommand::SetPermissionMode { run, mode } => {
                let reach = self.runs.control(run, &principal, |handle| handle.set_mode(mode));
                Self::reached(&principal, outbox, "set_permission_mode", run, reach);
            }
            ClientCommand::Hello { version } => outbox.send(if version == PROTOCOL_VERSION {
                ServerEvent::Welcome { version, principal: principal.clone() }
            } else {
                ServerEvent::error(format!("this server speaks protocol {PROTOCOL_VERSION}, the client {version}"))
            }),
            ClientCommand::ListModels => {
                let mut models = self.services.engines.models();
                if let Some(folder) = &self.services.local_models {
                    models.extend(superfik_gateway::mlx::downloaded(folder, superfik_core::EngineKind::Native));
                }
                self.services.providers.discover_models(&principal, models, self.services.engines.infos(), outbox);
            }
            ClientCommand::InspectWorkspace { path } => workspace::inspect(&self.services, outbox, &path),
            ClientCommand::SwitchBranch { path, branch } => {
                workspace::switch_branch(&self.services, outbox, &path, &branch)
            }
            ClientCommand::ListSkills { path } => {
                let mut skills = self.services.skills.skills(&path);
                skills.extend(self.services.plugins.skills(&principal));
                skills.extend(self.services.extensions.skills());
                outbox.send(ServerEvent::Skills(skills))
            }
            ClientCommand::ListAgents { path } => {
                let mut agents = self.services.agents.agents(&path);
                agents.extend(self.services.plugins.agents(&principal));
                agents.extend(self.services.extensions.agents());
                outbox.send(ServerEvent::Agents(agents))
            }
            ClientCommand::InspectRepository { path } => workspace::inspect_repository(&self.services, outbox, &path),
            ClientCommand::LoadMemory { path } => outbox.send(self.services.memory.load(&principal, &path)),
            ClientCommand::OptimizeMemory { run, path, model } => {
                let request = self.services.memory.optimization(&principal, &path, model);
                self.submit(&principal, run, &request)
            }
            ClientCommand::UndoMemory { path } => outbox.send(self.services.memory.undo(&principal, &path)),
            ClientCommand::SearchMemory { path, query } => {
                outbox.send(self.services.memory.search_event(&principal, &path, &query))
            }
            ClientCommand::EditMemory { path, edits } => {
                let by = Provenance { author: Author::User, now: memory::now(), origin: None };
                outbox.send(self.services.memory.edit(&principal, &path, &edits, by))
            }
            ClientCommand::LoadSessions { path, .. } => outbox.send(self.services.sessions.load(&principal, &path)),
            ClientCommand::SaveSession { path, session, .. } => {
                if let Some(error) = self.services.sessions.save(&principal, &path, &session) {
                    outbox.send(error);
                }
            }
            ClientCommand::ListPlugins => outbox.send(self.services.plugins.list(&principal)),
            ClientCommand::LoadSettings => {
                outbox.send(ServerEvent::Settings { settings: self.services.settings.load(&principal) });
            }
            ClientCommand::SaveSettings { settings } => {
                if let Err(error) = self.services.settings.save(&principal, &settings) {
                    tracing::warn!(%error, "could not keep settings");
                    outbox.send(ServerEvent::error(format!("Could not save the settings: {error}")));
                }
            }
            ClientCommand::InstallPlugin { folder } => {
                self.services.plugins.install_and_set_up(&principal, &folder, outbox)
            }
            ClientCommand::UninstallPlugin { plugin } => {
                outbox.send(self.services.plugins.uninstall(&principal, &plugin));
            }
            ClientCommand::Connect { plugin, label, values } => {
                self.services.plugins.connect(&principal, &plugin, &label, &values, outbox);
            }
            ClientCommand::Disconnect { connection } => {
                outbox.send(self.services.plugins.disconnect(&principal, connection));
            }
            ClientCommand::ListProviders => outbox.send(self.services.providers.list(&principal)),
            ClientCommand::DownloadModel { id } => {
                crate::models::download(self.services.downloads.as_ref(), id, outbox)
            }
            ClientCommand::ConnectProvider { provider, key } => {
                outbox.send(self.services.providers.connect(&principal, provider, &key))
            }
            ClientCommand::SignInProvider { provider } => self.services.providers.sign_in(&principal, provider, outbox),
            ClientCommand::DisconnectProvider { provider } => {
                outbox.send(self.services.providers.disconnect(&principal, provider))
            }
        }
    }

    pub fn cancel_all(&self) {
        self.runs.cancel_all();
    }

    pub fn active_runs(&self) -> usize {
        self.runs.len()
    }

    fn submit(&self, owner: &Principal, run: RunId, request: &PromptRequest) {
        if self.runs.is_active(run) {
            self.outbox.answering(owner).send(ServerEvent::error(format!("run {run} is already in progress")));
            return;
        }
        if let Err(message) = self.start(owner, run, request) {
            self.fail(run, message);
        }
    }

    fn start(&self, owner: &Principal, run: RunId, request: &PromptRequest) -> Result<(), String> {
        let outbox = self.outbox.answering(owner);
        let model = crate::native::key_for(&self.services.providers, owner, &request.model);
        let request = &PromptRequest {
            credentials: superfik_core::credentials::RunCredentials {
                github: None,
                model,
                owner: Some(owner.clone()),
                model_guard: None,
                mcp: Vec::new(),
            },
            ..request.clone()
        };
        let engine = self.services.engines.require(request.model.engine).map_err(|error| error.to_string())?;
        let request = self.services.plugins.brief(owner, run, request)?;
        let request = self.services.extensions.brief(run, request)?;
        let tool = request
            .memory
            .as_ref()
            .filter(|brief| !brief.optimizing)
            .and(self.memory_tool.as_ref())
            .map(|server| server.grant(owner, request.cwd.clone()));
        let request = self.services.memory.brief(owner, run, &request, tool);
        let sink = Arc::new(RunSink {
            runs: Arc::clone(&self.runs),
            outbox,
            memory: request.memory.is_some().then(|| (self.services.memory.clone(), owner.clone(), request.clone())),
            memory_tool: self.memory_tool.clone(),
            plugins: request.plugins.clone(),
        });
        match engine.start(run, &request, sink) {
            Ok(handle) => {
                self.runs.insert(run, owner.clone(), handle);
                Ok(())
            }
            Err(error) => {
                plugins::discard(request.plugins.as_ref());
                if let (Some(server), Some(tool)) =
                    (&self.memory_tool, request.memory.as_ref().and_then(|m| m.tool.as_ref()))
                {
                    server.revoke(&tool.token);
                }
                Err(error.to_string())
            }
        }
    }

    fn reached(principal: &Principal, outbox: &Outbox, command: &'static str, run: RunId, reach: Reach) {
        match reach {
            Reach::Reached => {}
            Reach::Inactive => tracing::debug!(%run, command, "for a run that is not active"),
            Reach::NotOwner => {
                let error = crate::authorizer::forbidden(principal, command);
                tracing::warn!(%run, %error, "someone else's run");
                outbox.send(ServerEvent::error(error.to_string()));
            }
        }
    }

    fn fail(&self, run: RunId, message: String) {
        self.outbox.send(ServerEvent::Run { run, event: EngineEvent::Failed(message) });
    }
}

struct RunSink {
    runs: Arc<RunTable>,
    outbox: Outbox,
    memory: Option<(Memories, Principal, PromptRequest)>,
    memory_tool: Option<McpServer>,
    plugins: Option<PluginBrief>,
}

impl EventSink for RunSink {
    fn emit(&self, run: RunId, event: EngineEvent) {
        if event.is_terminal() {
            self.runs.finish(run);
            plugins::discard(self.plugins.as_ref());
            let token = self.memory.as_ref().and_then(|(_, _, r)| r.memory.as_ref()?.tool.as_ref());
            if let (Some(server), Some(tool)) = (&self.memory_tool, token) {
                server.revoke(&tool.token);
            }
            let completed = matches!(event, EngineEvent::Completed { .. });
            if let Some(memory) =
                self.memory.as_ref().and_then(|(memories, owner, request)| memories.finish(owner, request, completed))
            {
                self.outbox.send(memory);
            }
        }
        self.outbox.send(ServerEvent::Run { run, event });
    }
}
