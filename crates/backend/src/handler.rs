use std::sync::Arc;

use agent_engine::EventSink;
use domain::{Author, EngineEvent, Principal, PromptRequest, Provenance, RunId};
use protocol::{ClientCommand, Envelope, ServerEvent};

use crate::memory::mcp::MemoryEndpoint;
use crate::memory::{self, Memories};
use crate::outbox::Outbox;
use crate::runs::{Reach, RunTable};
use crate::services::LocalServices;

#[derive(Debug)]
pub struct CommandHandler {
    services: LocalServices,
    runs: Arc<RunTable>,
    outbox: Outbox,
    client: u64,
}

impl Drop for CommandHandler {
    fn drop(&mut self) {
        crate::channels::close(self.client);
        agent_engine::tools::shells::kill_client(self.client);
        agent_engine::tools::peers::forget(self.client);
        agent_engine::tools::schedules::forget_client(self.client);
        agent_engine::tools::inbox::forget_client(self.client);
    }
}

impl CommandHandler {
    pub fn new(mut services: LocalServices, outbox: Outbox) -> Self {
        services.providers = services.providers.for_client();
        let runs = Arc::new(RunTable::default());
        static CLIENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let client = CLIENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { services, runs, outbox, client }
    }

    pub fn handle(&self, envelope: Envelope<ClientCommand>) {
        let Envelope { principal, payload: command } = envelope;
        let outbox = &self.outbox;
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
            ClientCommand::SteerRun { run, text } => {
                let reach = self.runs.control(run, &principal, |handle| handle.steer(text));
                Self::reached(&principal, outbox, "steer_run", run, reach);
            }
            ClientCommand::UnsteerRun { run } => {
                let reach = self.runs.control(run, &principal, |handle| handle.unsteer());
                Self::reached(&principal, outbox, "unsteer_run", run, reach);
            }
            ClientCommand::BackgroundRun { run } => {
                let reach = self.runs.control(run, &principal, |handle| handle.background());
                Self::reached(&principal, outbox, "background_run", run, reach);
            }
            ClientCommand::ListModels => {
                let models = self.services.engine.models();
                self.services.providers.discover_models(&principal, models, outbox);
            }
            ClientCommand::ListSkills { path } => {
                agent_engine::tools::peers::register(self.client, Some(&principal), &path);
                outbox.send(ServerEvent::Skills(crate::extensions::skills(&path, &self.services.extensions)));
            }
            ClientCommand::ListAgents { path } => {
                outbox.send(ServerEvent::Agents(crate::extensions::agents(&path, &self.services.extensions)));
            }
            ClientCommand::LoadMemory { path } => outbox.send(self.services.memory.load(&principal, &path)),
            ClientCommand::SearchMemory { path, query } => {
                outbox.send(self.services.memory.search_event(&principal, &path, &query))
            }
            ClientCommand::EditMemory { path, edits } => {
                let by = Provenance { author: Author::User, now: memory::now(), origin: None };
                outbox.send(self.services.memory.edit(&principal, &path, &edits, by))
            }
            ClientCommand::OptimizeMemory { run, path, model } => {
                let request = self.services.memory.optimization(&principal, &path, model);
                self.submit(&principal, run, &request);
            }
            ClientCommand::UndoMemory { path } => outbox.send(self.services.memory.undo(&principal, &path)),
            ClientCommand::LoadSettings => {
                outbox.send(ServerEvent::Settings { settings: self.services.settings.load(&principal) });
            }
            ClientCommand::SaveSettings { settings } => {
                if let Err(error) = self.services.settings.save(&principal, &settings) {
                    tracing::warn!(%error, "could not keep settings");
                    outbox.send(ServerEvent::error(format!("Could not save the settings: {error}")));
                }
            }
            ClientCommand::ListProviders => outbox.send(self.services.providers.list(&principal)),
            ClientCommand::ConnectProvider { provider, key } => {
                outbox.send(self.services.providers.connect(&principal, provider, &key))
            }
            ClientCommand::SignInProvider { provider } => self.services.providers.sign_in(&principal, provider, outbox),
            ClientCommand::DisconnectProvider { provider } => {
                outbox.send(self.services.providers.disconnect(&principal, provider))
            }
            ClientCommand::ListMcpServers { path } => self.mcp_servers(&path, Ok(())),
            ClientCommand::ListShells { kill } => {
                if let Some(id) = kill {
                    agent_engine::tools::shells::kill(self.client, &id);
                }
                outbox.send(ServerEvent::Shells(agent_engine::tools::shells::list(self.client)));
            }
            ClientCommand::Schedules { delete, take_due } => {
                use agent_engine::tools::schedules;
                if let Some(id) = delete {
                    schedules::remove(self.client, &id);
                }
                let (due, notices) = if take_due {
                    (schedules::take_due(self.client, schedules::now()), agent_engine::tools::inbox::take(self.client))
                } else {
                    (Vec::new(), Vec::new())
                };
                outbox.send(ServerEvent::Schedules { due, scheduled: schedules::list(self.client), notices });
            }
            ClientCommand::ListPermissions { path } => self.outbox.send(crate::permissions::list(
                &path,
                &self.services.extensions,
                self.services.home.as_deref(),
            )),
            ClientCommand::ApproveMcpServer { path, name, approve } => {
                self.mcp_servers(&path, crate::mcp_admin::approve(&path, self.services.home.as_deref(), &name, approve))
            }
            ClientCommand::AddMcpServer { path, name, server } => {
                self.mcp_servers(&path, crate::mcp_admin::add(self.services.home.as_deref(), &name, server))
            }
            ClientCommand::RemoveMcpServer { path, name } => {
                self.mcp_servers(&path, crate::mcp_admin::remove(self.services.home.as_deref(), &name))
            }
            ClientCommand::SignInMcpServer { path, name } => {
                crate::mcp_auth::command::sign_in(&self.services, &principal, path, name, &self.outbox);
            }
            ClientCommand::SignOutMcpServer { path, name } => {
                crate::mcp_auth::command::sign_out(&self.services, &principal, &path, &name, &self.outbox);
            }
            ClientCommand::OpenChannels { path, names } => outbox.send(crate::channels::open_here(
                self.client,
                &path,
                &names,
                &self.services.extensions,
                self.services.home.as_deref(),
            )),
        }
    }

    fn mcp_servers(&self, path: &std::path::Path, changed: Result<(), String>) {
        let event = match changed {
            Ok(()) => crate::mcp_admin::list(path, &self.services.extensions, self.services.home.as_deref()),
            Err(error) => ServerEvent::error(error),
        };
        self.outbox.send(event);
    }

    pub fn cancel_all(&self) {
        self.runs.cancel_all();
    }

    fn submit(&self, owner: &Principal, run: RunId, request: &PromptRequest) {
        if self.runs.is_active(run) {
            self.outbox.send(ServerEvent::error(format!("run {run} is already in progress")));
            return;
        }
        if let Err(message) = self.start(owner, run, request) {
            self.fail(run, message);
        }
    }

    fn start(&self, owner: &Principal, run: RunId, request: &PromptRequest) -> Result<(), String> {
        agent_engine::tools::peers::register(self.client, Some(owner), &request.cwd);
        let outbox = self.outbox.clone();
        let model = crate::native::key_for(&self.services.providers, owner, &request.model);
        let request = PromptRequest {
            credentials: domain::credentials::RunCredentials {
                model,
                owner: Some(owner.clone()),
                mcp: crate::channels::for_run(self.client),
                client: self.client,
                mcp_tokens: Some(self.services.mcp_auth.for_run(owner)),
            },
            ..request.clone()
        };
        let mut request = self.services.extensions.brief(run, request)?;
        let optimizing = request.memory.as_ref().is_some_and(|brief| brief.optimizing);
        let tool = self.services.memory_tool && request.memory.is_some() && !optimizing;
        if tool {
            let memories = self.services.memory.clone();
            request.credentials.mcp.push(MemoryEndpoint::for_run(memories, owner.clone(), request.cwd.clone()));
        }
        let request = self.services.memory.brief(owner, run, &request, tool);
        let sink = Arc::new(RunSink {
            runs: Arc::clone(&self.runs),
            outbox,
            memory: request.memory.is_some().then(|| (self.services.memory.clone(), owner.clone(), request.clone())),
            extensions: request.extensions.clone(),
        });
        match self.services.engine.start(run, &request, sink) {
            Ok(handle) => {
                self.runs.insert(run, owner.clone(), handle);
                Ok(())
            }
            Err(error) => {
                crate::extensions::discard(request.extensions.as_ref());
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
    extensions: Option<domain::Extensions>,
}

impl EventSink for RunSink {
    fn emit(&self, run: RunId, event: EngineEvent) {
        if event.is_terminal() {
            self.runs.finish(run);
            crate::extensions::discard(self.extensions.as_ref());
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
