use std::sync::Arc;

use agent_engine::EventSink;
use domain::{EngineEvent, PromptRequest, RunId};
use platform::sync::Locked;
use protocol::{ClientCommand, Envelope, ServerEvent};

use crate::feature::{Context, Finish};
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
        agent_engine::tools::peers::forget(self.client);
        crate::features::forget(self.client);
        agent_engine::tools::inbox::forget_client(self.client);
        agent_engine::tools::watch::forget_client(self.client);
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

    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "split planned: one match over every client command, to become a handler per command"
    )]
    pub fn handle(&self, envelope: Envelope<ClientCommand>) {
        let outbox = &self.outbox;
        if let Err(error) = envelope.spoken() {
            tracing::warn!(%error, "command in another protocol version");
            outbox.send(ServerEvent::error(error));
            return;
        }
        let command = envelope.payload;
        tracing::debug!(command = command.name(), "handling command");
        match command {
            ClientCommand::SubmitPrompt { run, request } => {
                self.submit(run, &crate::sent::as_sent(&request));
            }
            ClientCommand::CancelRun { run } => {
                Self::reached("cancel_run", run, self.runs.cancel(run));
            }
            ClientCommand::AnswerAsk { run, ask, decision } => {
                let reach = self.runs.control(run, |handle| handle.answer(&ask, &decision));
                Self::reached("answer_ask", run, reach);
            }
            ClientCommand::SetPermissionMode { run, mode } => {
                let reach = self.runs.control(run, |handle| handle.set_mode(mode));
                Self::reached("set_permission_mode", run, reach);
            }
            ClientCommand::SteerRun { run, text } => {
                let reach = self.runs.control(run, |handle| handle.steer(text));
                Self::reached("steer_run", run, reach);
            }
            ClientCommand::UnsteerRun { run } => {
                let reach = self.runs.control(run, |handle| handle.unsteer());
                Self::reached("unsteer_run", run, reach);
            }
            ClientCommand::BackgroundRun { run } => {
                let reach = self.runs.control(run, |handle| handle.background());
                Self::reached("background_run", run, reach);
            }
            ClientCommand::ListModels => {
                let models = self.services.engine.models();
                self.services.providers.discover_models(models, outbox);
            }
            ClientCommand::ListSkills { path } => {
                agent_engine::tools::peers::register(self.client, &path);
                outbox.send(ServerEvent::Skills(crate::extensions::skills(
                    &path,
                    &self.services.extensions,
                    self.services.home().as_deref(),
                )));
            }
            ClientCommand::ListAgents { path } => {
                outbox.send(ServerEvent::Agents(crate::extensions::agents(
                    &path,
                    &self.services.extensions,
                    self.services.home().as_deref(),
                )));
            }
            ClientCommand::LoadSettings => {
                outbox.send(ServerEvent::Settings { settings: self.services.settings.load() });
            }
            ClientCommand::SaveSettings { settings } => {
                if let Err(error) = self.services.settings.save(&settings) {
                    tracing::warn!(%error, "could not keep settings");
                    outbox.send(ServerEvent::error(format!("Could not save the settings: {error}")));
                }
            }
            ClientCommand::ListProviders => outbox.send(self.services.providers.list()),
            ClientCommand::ConnectProvider { provider, key } => {
                outbox.send(self.services.providers.connect(provider, &key))
            }
            ClientCommand::SignInProvider { provider } => self.services.providers.sign_in(provider, outbox),
            ClientCommand::CloseAndSignIn { provider, port, pid } => {
                self.services.providers.close_and_sign_in(provider, port, pid, outbox)
            }
            ClientCommand::DisconnectProvider { provider } => outbox.send(self.services.providers.disconnect(provider)),
            ClientCommand::UseProviderAccount { provider, account } => {
                outbox.send(self.services.providers.use_account(provider, &account))
            }
            ClientCommand::RemoveProviderAccount { provider, account } => {
                outbox.send(self.services.providers.remove_account(provider, &account))
            }
            ClientCommand::SetProviderBalance { provider, balance } => {
                outbox.send(self.services.providers.set_balance(provider, balance))
            }
            ClientCommand::ListMcpServers { path } => self.mcp_servers(&path, Ok(())),
            ClientCommand::TakeNotices => {
                let unkept = self.services.providers.unkept_notice();
                let arrived = agent_engine::tools::inbox::take(self.client);
                outbox.send(ServerEvent::Notices(unkept.into_iter().chain(arrived).collect()));
            }
            ClientCommand::Feature { feature, input } => {
                let output = self
                    .with_context(|context| crate::features::handle(&feature, context, input))
                    .unwrap_or_else(|| Err(format!("there is no {feature} here")));
                outbox.send(ServerEvent::Feature { feature, output });
            }
            ClientCommand::ListPermissions { path } => self.outbox.send(crate::permissions::list(
                &path,
                &self.services.extensions,
                self.services.home().as_deref(),
            )),
            ClientCommand::ApproveMcpServer { path, name, approve } => self
                .mcp_servers(&path, crate::mcp_admin::approve(&path, self.services.home().as_deref(), &name, approve)),
            ClientCommand::AddMcpServer { path, name, server } => {
                self.mcp_servers(&path, crate::mcp_admin::add(self.services.home().as_deref(), &name, server))
            }
            ClientCommand::RemoveMcpServer { path, name } => {
                self.mcp_servers(&path, crate::mcp_admin::remove(self.services.home().as_deref(), &name))
            }
            ClientCommand::SignInMcpServer { path, name } => {
                crate::mcp_auth::command::sign_in(&self.services, path, name, &self.outbox);
            }
            ClientCommand::SignOutMcpServer { path, name } => {
                crate::mcp_auth::command::sign_out(&self.services, &path, &name, &self.outbox);
            }
        }
    }

    fn mcp_servers(&self, path: &std::path::Path, changed: Result<(), String>) {
        let event = match changed {
            Ok(()) => crate::mcp_admin::list(path, &self.services.extensions, self.services.home().as_deref()),
            Err(error) => ServerEvent::error(error),
        };
        self.outbox.send(event);
    }

    pub fn cancel_all(&self) {
        self.runs.cancel_all();
    }

    fn with_context<T>(&self, body: impl FnOnce(&Context<'_>) -> T) -> T {
        let submit = |run: RunId, request: PromptRequest| {
            self.submit(run, &request);
            Ok(())
        };
        body(&Context { client: self.client, services: &self.services, submit: &submit })
    }

    fn submit(&self, run: RunId, request: &PromptRequest) {
        if self.runs.is_active(run) {
            self.outbox.send(ServerEvent::error(format!("run {run} is already in progress")));
            return;
        }
        if let Err(message) = self.start(run, request) {
            self.fail(run, message);
        }
    }

    fn start(&self, run: RunId, request: &PromptRequest) -> Result<(), String> {
        agent_engine::tools::peers::register(self.client, &request.cwd);
        let outbox = self.outbox.clone();
        let (model_accounts, mut keys): (Vec<String>, Vec<_>) =
            crate::native::keys_for(&self.services.providers, &request.model).into_iter().unzip();
        let model = (!keys.is_empty()).then(|| keys.remove(0));
        let request = PromptRequest {
            credentials: domain::credentials::RunCredentials {
                model,
                model_pool: keys,
                model_accounts,
                other_models: crate::native::other_keys(&self.services.providers, &request.model),
                mcp: Vec::new(),
                client: self.client,
                mcp_tokens: Some(self.services.mcp_auth.for_run()),
            },
            ..request.clone()
        };
        let request = self.services.extensions.brief(request, self.services.home());
        let (request, finishes) = self.with_context(|context| crate::features::prepare(context, run, request));
        let sink = Arc::new(RunSink {
            runs: Arc::clone(&self.runs),
            listing: crate::catalog::Listing::of(&self.services, &request),
            outbox,
            finishes: std::sync::Mutex::new(finishes),
            outcome: std::sync::Mutex::default(),
        });
        match self.services.engine.start(run, &request, sink) {
            Ok(handle) => {
                self.runs.insert(run, handle);
                Ok(())
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn reached(command: &'static str, run: RunId, reach: Reach) {
        if reach == Reach::Inactive {
            tracing::debug!(%run, command, "for a run that is not active");
        }
    }

    fn fail(&self, run: RunId, message: String) {
        self.outbox.send(ServerEvent::Run { run, event: EngineEvent::Failed(message) });
    }
}

#[cfg(test)]
#[path = "handler_tests.rs"]
mod tests;

struct RunSink {
    runs: Arc<RunTable>,
    listing: Option<crate::catalog::Listing>,
    outbox: Outbox,
    finishes: std::sync::Mutex<Vec<Finish>>,
    outcome: std::sync::Mutex<domain::memory::scorecard::RunOutcome>,
}

impl EventSink for RunSink {
    fn emit(&self, run: RunId, event: EngineEvent) {
        if let Some(listing) = &self.listing {
            listing.observe(&event);
        }
        self.outcome.locked().observe(&event);
        if event.is_terminal() {
            self.runs.finish(run);
            let outcome = *self.outcome.locked();
            let finishes = std::mem::take(&mut *self.finishes.locked());
            for finished in finishes.into_iter().filter_map(|finish| finish(&outcome)) {
                self.outbox.send(finished);
            }
        }
        self.outbox.send(ServerEvent::Run { run, event });
    }
}
