mod flows;
pub mod local;
pub mod outbox;
mod runs;
mod sent;

use std::sync::Arc;

use crate::api::{ClientCommand, Envelope, Notices, Reply, ServerEvent};
use agent_engine::EventSink;
use domain::{EngineEvent, PromptRequest, RunId};
use platform::sync::Locked;

use crate::features::context::{Context, Finish};
use crate::handler::flows::Flows;
use crate::handler::outbox::{Outbox, Replies};
use crate::handler::runs::{Reach, RunTable};
use crate::services::LocalServices;
use crate::trust::Asked;

#[derive(Debug)]
pub struct CommandHandler {
    services: LocalServices,
    runs: Arc<RunTable>,
    outbox: Outbox,
    flows: Flows,
    client: u64,
}

impl Drop for CommandHandler {
    fn drop(&mut self) {
        agent_engine::tools::peers::forget(self.client);
        crate::features::forget(self.client);
        agent_engine::tools::inbox::forget_client(self.client);
        agent_engine::tools::watch::forget_client(self.client);
        agent_engine::team::lead_inbox::closing(self.client);
    }
}

impl CommandHandler {
    pub fn new(services: LocalServices, outbox: Outbox) -> Self {
        let runs = Arc::new(RunTable::default());
        static CLIENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let client = CLIENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { services, runs, outbox, flows: Flows::default(), client }
    }

    pub fn handle(&self, envelope: Envelope<ClientCommand>) {
        let replies = self.outbox.answering(envelope.id);
        let command = envelope.payload;
        let name = command.name();
        tracing::debug!(command = name, request = %envelope.id, "handling command");
        let answered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.answer(command, &replies)));
        match answered {
            Ok(Some(answer)) => replies.send(answer),
            Ok(None) => {}
            Err(_) => {
                tracing::error!(command = name, request = %envelope.id, "a command's handler panicked");
                replies.send(Reply::error(format!("internal error while handling {name}")));
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "split planned: one match over every client command, to become a handler per command"
    )]
    fn answer(&self, command: ClientCommand, replies: &Replies) -> Option<Reply> {
        let providers = &self.services.providers;
        let home = self.services.home();
        let home = home.as_deref();
        Some(match command {
            ClientCommand::SubmitPrompt { run, request } => {
                Reply::done_or(self.submit(run, &crate::handler::sent::as_sent(&request)))
            }
            ClientCommand::CancelRun { run } => Self::reached("cancel_run", run, self.runs.cancel(run)),
            ClientCommand::AnswerAsk { run, ask, decision } => {
                let reach = self.runs.control(run, |handle| handle.answer(&ask, &decision));
                Self::reached("answer_ask", run, reach)
            }
            ClientCommand::SetPermissionMode { run, mode } => {
                let reach = self.runs.control(run, |handle| handle.set_mode(mode));
                Self::reached("set_permission_mode", run, reach)
            }
            ClientCommand::SteerRun { run, text } => {
                let reach = self.runs.control(run, |handle| handle.steer(text));
                Self::reached("steer_run", run, reach)
            }
            ClientCommand::UnsteerRun { run } => {
                let reach = self.runs.control(run, |handle| handle.unsteer());
                Self::reached("unsteer_run", run, reach)
            }
            ClientCommand::BackgroundRun { run } => {
                let reach = self.runs.control(run, |handle| handle.background());
                Self::reached("background_run", run, reach)
            }
            ClientCommand::ListModels => {
                providers.discover_models(self.services.engine.models(), replies);
                return None;
            }
            ClientCommand::RegisterPeer { path } => {
                agent_engine::tools::peers::register(self.client, &path);
                Reply::Done
            }
            ClientCommand::ListSkills { path } => {
                Reply::Skills(crate::extensions::skills(&path, &self.services.extensions, home))
            }
            ClientCommand::ListAgents { path } => {
                Reply::Agents(crate::extensions::agents(&path, &self.services.extensions, home))
            }
            ClientCommand::LoadSettings => Reply::Settings { settings: self.services.settings.load() },
            ClientCommand::SaveSettings { settings } => {
                Reply::done_or(self.services.settings.save(&settings).map_err(|error| {
                    tracing::warn!(%error, "could not keep settings");
                    format!("Could not save the settings: {error}")
                }))
            }
            ClientCommand::ChangeSettings { changes } => {
                Reply::done_or(self.services.settings.change(&changes).map_err(|error| {
                    tracing::warn!(%error, "could not keep the changed settings");
                    format!("Could not save the settings: {error}")
                }))
            }
            ClientCommand::ListProviders => providers.list(),
            ClientCommand::ConnectProvider { provider, key } => providers.connect(provider, &key),
            ClientCommand::SignInProvider { provider } => {
                self.flows.began(replies);
                providers.sign_in(provider, replies);
                return None;
            }
            ClientCommand::CloseAndSignIn { provider, port, pid } => {
                self.flows.began(replies);
                providers.close_and_sign_in(provider, port, pid, replies);
                return None;
            }
            ClientCommand::DisconnectProvider { provider } => providers.disconnect(provider),
            ClientCommand::UseProviderAccount { provider, account } => providers.use_account(provider, &account),
            ClientCommand::RemoveProviderAccount { provider, account } => providers.remove_account(provider, &account),
            ClientCommand::SetProviderBalance { provider, balance } => providers.set_balance(provider, balance),
            ClientCommand::ListMcpServers { path } => self.mcp_servers(&path, Ok(())),
            ClientCommand::TakeNotices { for_agent } => Reply::Notices(self.notices(for_agent)),
            ClientCommand::AnswerTeammate { ask, decision } => {
                Reply::done_or(agent_engine::team::lead_inbox::answer(self.client, &ask, &decision))
            }
            ClientCommand::Feature(request) => {
                match self.with_context(|context| crate::features::handle(context, request)) {
                    Ok(reply) => Reply::Feature(reply),
                    Err(error) => Reply::FeatureFailed(error),
                }
            }
            ClientCommand::ListPermissions { path } => crate::permissions::list(&path, &self.services.extensions, home),
            ClientCommand::ApproveMcpServer { path, name, approve } => {
                let extensions = &self.services.extensions;
                let approved = crate::mcp_admin::known(&path, extensions, home, &name)
                    .and_then(|()| crate::mcp_admin::approve(&path, home, &name, approve));
                self.mcp_servers(&path, approved)
            }
            ClientCommand::AddMcpServer { path, name, server, replace } => {
                self.mcp_servers(&path, crate::mcp_admin::add(home, &name, server, replace))
            }
            ClientCommand::GetMcpServer { path, name } => {
                crate::mcp_admin::get(&path, &self.services.extensions, home, &name)
            }
            ClientCommand::RemoveMcpServer { path, name } => {
                self.mcp_servers(&path, crate::mcp_admin::remove(home, &name))
            }
            ClientCommand::SignInMcpServer { path, name } => {
                self.flows.began(replies);
                crate::mcp_auth::command::sign_in(&self.services, path, name, replies);
                return None;
            }
            ClientCommand::SignOutMcpServer { path, name } => {
                crate::mcp_auth::command::sign_out(&self.services, &path, &name)
            }
            ClientCommand::Trust { path, trusted } => crate::trust::answer(&self.services, &path, Asked::Kept(trusted)),
            ClientCommand::TrustForThisProcess { path } => {
                crate::trust::answer(&self.services, &path, Asked::ForThisProcess)
            }
            ClientCommand::TrustState { path } => crate::trust::answer(&self.services, &path, Asked::State),
            ClientCommand::CancelRequest { request } => {
                if !self.flows.cancel(request) {
                    tracing::debug!(%request, "nothing left to cancel for this request");
                }
                Reply::Done
            }
        })
    }

    fn notices(&self, for_agent: bool) -> Notices {
        let unkept_keys = self.services.providers.unkept_notice();
        let unkept_database = self.services.database.told_once(crate::degraded::database_notice);
        let arrived = if for_agent { agent_engine::tools::inbox::take(self.client) } else { Vec::new() };
        Notices {
            for_user: unkept_keys.into_iter().chain(unkept_database).collect(),
            for_agent: arrived,
            teammate_asks: agent_engine::team::lead_inbox::take_asks(self.client),
        }
    }

    fn mcp_servers(&self, path: &std::path::Path, changed: Result<(), String>) -> Reply {
        match changed {
            Ok(()) => crate::mcp_admin::list(path, &self.services.extensions, self.services.home().as_deref()),
            Err(error) => Reply::error(error),
        }
    }

    pub fn cancel_all(&self) {
        self.runs.cancel_all();
    }

    fn with_context<T>(&self, body: impl FnOnce(&Context<'_>) -> T) -> T {
        let submit = |run: RunId, request: PromptRequest| self.submit(run, &request);
        body(&Context { client: self.client, services: &self.services, submit: &submit })
    }

    fn submit(&self, run: RunId, request: &PromptRequest) -> Result<(), String> {
        if self.runs.is_active(run) {
            return Err(format!("run {run} is already in progress"));
        }
        if let Err(message) = self.start(run, request) {
            self.fail(run, message);
        }
        Ok(())
    }

    fn start(&self, run: RunId, request: &PromptRequest) -> Result<(), String> {
        agent_engine::tools::peers::register(self.client, &request.session.cwd);
        let outbox = self.outbox.clone();
        let (model_accounts, mut keys): (Vec<String>, Vec<_>) =
            crate::engine::keys_for(&self.services.providers, &request.session.model).into_iter().unzip();
        let model = (!keys.is_empty()).then(|| keys.remove(0));
        let mut request = request.clone();
        request.host.credentials = domain::credentials::RunCredentials {
            model,
            model_pool: keys,
            model_accounts,
            other_models: crate::engine::other_keys(&self.services.providers, &request.session.model),
            mcp: Vec::new(),
            client: self.client,
            mcp_tokens: Some(self.services.mcp_auth.for_run()),
        };
        let request = self.services.extensions.brief(request, self.services.home());
        let (request, finishes) = self.with_context(|context| crate::features::prepare(context, run, request));
        let sink = Arc::new(RunSink {
            runs: Arc::clone(&self.runs),
            listing: crate::catalog::Listing::of(&self.services, &request),
            outbox,
            finishes: std::sync::Mutex::new(finishes),
            outcome: std::sync::Mutex::default(),
            exported: self
                .services
                .exporter
                .as_ref()
                .map(|exporter| exporter.watch(&request.session.cwd, request.session.resume.as_ref())),
        });
        match self.services.engine.start(run, &request, sink) {
            Ok(handle) => {
                self.runs.insert(run, handle);
                Ok(())
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn reached(command: &'static str, run: RunId, reach: Reach) -> Reply {
        if reach == Reach::Inactive {
            tracing::debug!(%run, command, "for a run that is not active");
        }
        Reply::Done
    }

    fn fail(&self, run: RunId, message: String) {
        self.outbox.send(ServerEvent::Run { run, event: EngineEvent::Failed(message) });
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod trust_tests;

struct RunSink {
    runs: Arc<RunTable>,
    listing: Option<crate::catalog::Listing>,
    outbox: Outbox,
    finishes: std::sync::Mutex<Vec<Finish>>,
    outcome: std::sync::Mutex<domain::memory::scorecard::RunOutcome>,
    exported: Option<crate::features::trace::export::Watch>,
}

impl EventSink for RunSink {
    fn emit(&self, run: RunId, event: EngineEvent) {
        if let Some(listing) = &self.listing {
            listing.observe(&event);
        }
        if let Some(exported) = &self.exported {
            exported.observe(&event);
        }
        self.outcome.locked().observe(&event);
        if event.is_terminal() {
            self.runs.finish(run);
            let outcome = *self.outcome.locked();
            let finishes = std::mem::take(&mut *self.finishes.locked());
            for finished in finishes.into_iter().filter_map(|finish| finish(&outcome)) {
                self.outbox.send(ServerEvent::Unsolicited(finished));
            }
        }
        self.outbox.send(ServerEvent::Run { run, event });
    }
}
