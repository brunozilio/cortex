//! Executes one authorized command. Synchronous and transport-free, so a
//! local backend and a future network server share it.

use std::sync::Arc;

use std::path::PathBuf;

use superfik_core::note::NotesTool;
use superfik_core::{
    Author, BrowserTool, EngineEvent, Handoff, HandoffKind, PluginBrief, Principal, PromptRequest, Provenance, RunId,
    VaultTool,
};
use superfik_engine::EventSink;
use superfik_protocol::{ClientCommand, Envelope, PROTOCOL_VERSION, ServerEvent};

use crate::browser::BrowserTools;
use crate::memory::mcp::McpServer;
use crate::memory::{self, Memories};
use crate::notes::mcp::{NotesTools, Publish};
use crate::outbox::Outbox;
use crate::plugins;
use crate::routines::Trace;

mod channel_links;
mod routine_firing;
mod routine_webhooks;

use crate::channels::mcp::ChannelTools;
use crate::runs::{Reach, RunTable};
use crate::services::LocalServices;
use crate::sign_in;
use crate::vault::{Logins, Notify, Vault, VaultAccount};
use crate::workspace;

#[derive(Debug)]
pub struct CommandHandler {
    services: LocalServices,
    runs: Arc<RunTable>,
    outbox: Outbox,
    memory_tool: Option<McpServer>,
    vault: Option<Vault>,
    browser: Option<BrowserTools>,
    notes_tool: Option<NotesTools>,
    channel_tools: Option<ChannelTools>,
    logins: Logins,
    webhooks: crate::routines::Webhooks,
}

impl CommandHandler {
    pub fn new(services: LocalServices, outbox: Outbox) -> Self {
        let memory_tool = services.memory_tool.then(|| McpServer::start(services.memory.clone())).flatten();
        let to_client = outbox.clone();
        let notify: Notify = Arc::new(move |run, event| to_client.send(ServerEvent::Run { run, event }));
        let vault = Vault::start(services.vaults.clone(), services.commands.clone(), notify.clone());
        let browser = BrowserTools::start(notify).map(|browser| match &vault {
            Some(vault) => browser.with_logins(Arc::new(vault.clone())),
            None => browser,
        });
        let to_owner = outbox.clone();
        let publish: Publish = Arc::new(move |owner, event| to_owner.answering(owner).send(event));
        let notes_tool = NotesTools::start(services.notes.clone(), Arc::clone(&publish));
        let channel_tools = ChannelTools::start(services.channels.clone(), publish);
        let logins = Logins::new(services.vaults.clone());
        let webhooks = crate::routines::Webhooks::new(services.plugins.secrets());
        let runs = Arc::new(RunTable::default());
        Self { services, runs, outbox, memory_tool, vault, browser, notes_tool, channel_tools, logins, webhooks }
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
                if self.vault.as_ref().is_some_and(|vault| vault.answer(&principal, &ask, &decision)) {
                    return;
                }
                let reach = self.runs.control(run, &principal, |handle| handle.answer(&ask, &decision));
                Self::reached(&principal, outbox, "answer_ask", run, reach);
            }
            ClientCommand::AnswerBrowse { run: _, id, answer } => {
                if let Some(browser) = &self.browser {
                    browser.answer(&principal, &id, answer);
                }
            }
            ClientCommand::Page(call) => self.services.pages.call(&principal, call, outbox.clone()),
            ClientCommand::SetPermissionMode { run, mode } => {
                let reach = self.runs.control(run, &principal, |handle| handle.set_mode(mode));
                Self::reached(&principal, outbox, "set_permission_mode", run, reach);
            }
            ClientCommand::Hello { version } => outbox.send(if version == PROTOCOL_VERSION {
                ServerEvent::Welcome { version, principal: principal.clone() }
            } else {
                ServerEvent::error(format!("this server speaks protocol {PROTOCOL_VERSION}, the app {version}"))
            }),
            ClientCommand::ListMembers
            | ClientCommand::AddUser { .. }
            | ClientCommand::NewFirstAccessCode { .. }
            | ClientCommand::RemoveUser { .. }
            | ClientCommand::SetUserRole { .. }
            | ClientCommand::LoadServerSettings
            | ClientCommand::SaveServerSettings { .. }
            | ClientCommand::DelegateAsk { .. }
            | ClientCommand::ListDelegated
            | ClientCommand::AnswerDelegated { .. }
            | ClientCommand::LoadSharedMemory { .. }
            | ClientCommand::ShareMemory { .. }
            | ClientCommand::UnshareMemory { .. }
            | ClientCommand::ListLibrary
            | ClientCommand::ListTeams
            | ClientCommand::LoadComments { .. }
            | ClientCommand::AddComment { .. }
            | ClientCommand::CreateTeam { .. }
            | ClientCommand::RemoveTeam { .. }
            | ClientCommand::SetTeamMember { .. }
            | ClientCommand::PublishToLibrary { .. }
            | ClientCommand::RemoveFromLibrary { .. }
            | ClientCommand::GetPolicy
            | ClientCommand::SetPolicy { .. }
            | ClientCommand::ReportUsage { .. }
            | ClientCommand::ListUsage => {
                outbox.send(ServerEvent::error("this computer is not part of an organization"));
            }
            ClientCommand::ListModels => {
                let mut models = self.services.engines.models();
                if let Some(folder) = &self.services.local_models {
                    models.extend(superfik_gateway::mlx::downloaded(folder, superfik_core::EngineKind::Native));
                }
                self.services.plugins.discover_models(
                    &principal,
                    &self.services.providers,
                    models,
                    self.services.engines.infos(),
                    outbox,
                );
            }
            ClientCommand::InspectWorkspace { path } => {
                workspace::inspect(&self.services.acting_as(&principal), outbox, &path)
            }
            ClientCommand::SwitchBranch { path, branch } => {
                workspace::switch_branch(&self.services.acting_as(&principal), outbox, &path, &branch)
            }
            ClientCommand::ListSkills { path } => {
                let mut skills = self.services.skills.skills(&path);
                skills.extend(self.services.plugins.skills(&principal));
                outbox.send(ServerEvent::Skills(skills))
            }
            ClientCommand::ListAgents { path } => {
                let mut agents = self.services.agents.agents(&path);
                agents.extend(self.services.plugins.agents(&principal));
                outbox.send(ServerEvent::Agents(agents))
            }
            ClientCommand::InspectRepository { path } => {
                workspace::inspect_repository(&self.services.acting_as(&principal), outbox, &path)
            }
            ClientCommand::ListGitHubRepositories => {
                workspace::list_github(&self.services.acting_as(&principal), outbox)
            }
            ClientCommand::LocateRepository { slug, known, clone_root } => {
                let services = self.services.acting_as(&principal);
                workspace::locate_repository(&services, outbox, &slug, &known, &clone_root)
            }
            ClientCommand::ListPullRequests { path } => {
                workspace::list_pull_requests(&self.services.acting_as(&principal), outbox, &path)
            }
            ClientCommand::CheckoutPullRequest { path, number } => {
                workspace::checkout_pull_request(&self.services.acting_as(&principal), outbox, &path, number)
            }
            ClientCommand::CreatePullRequest { path, draft } => {
                workspace::create_pull_request(&self.services.acting_as(&principal), outbox, &path, &draft)
            }
            ClientCommand::LoadPullRequest { path, number } => {
                workspace::load_pull_request(&self.services.acting_as(&principal), outbox, &path, number)
            }
            ClientCommand::SubmitReview { path, number, verdict, body, comments } => {
                let services = self.services.acting_as(&principal);
                workspace::submit_review(&services, outbox, &path, number, verdict, &body, &comments)
            }
            ClientCommand::ResolveThread { path, thread, resolved } => {
                workspace::resolve_thread(&self.services.acting_as(&principal), outbox, &path, &thread, resolved)
            }
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
            ClientCommand::LoadNotes { path } => outbox.send(self.services.notes.load(&principal, &path)),
            ClientCommand::EditNotes { path, edits } => {
                let by = superfik_core::note::Stamp { author: Author::User, now: memory::now() };
                outbox.send(self.services.notes.edit(&principal, &path, &edits, by, None))
            }
            ClientCommand::LoadRoutines { path } => outbox.send(self.services.routines.load(&principal, &path)),
            ClientCommand::EditRoutines { path, edits } => outbox.send(self.edit_routines(&principal, &path, &edits)),
            ClientCommand::RevealWebhook { path, routine } => {
                outbox.send(self.reveal_webhook(&principal, &path, routine));
            }
            ClientCommand::LoadSessions { path, .. } => outbox.send(self.services.sessions.load(&principal, &path)),
            ClientCommand::SaveSession { path, session, .. } => {
                if let Some(error) = self.services.sessions.save(&principal, &path, &session) {
                    outbox.send(error);
                }
            }
            ClientCommand::ListPlugins => outbox.send(self.services.plugins.list(&principal)),
            ClientCommand::Upload { .. } => {
                outbox.send(ServerEvent::error("this server reads your files where they are"));
            }
            ClientCommand::LoadSettings => {
                outbox.send(ServerEvent::Settings { settings: self.services.settings.load(&principal) });
            }
            ClientCommand::SaveSettings { settings } => {
                if let Err(error) = self.services.settings.save(&principal, &settings) {
                    tracing::warn!(%error, "could not keep settings");
                    outbox.send(ServerEvent::error(format!("Could not save the settings: {error}")));
                }
            }
            ClientCommand::SignInToGitHub | ClientCommand::SignOutOfGitHub => {
                outbox.send(ServerEvent::error("sign in to GitHub on this computer with `gh auth login`"));
            }
            ClientCommand::FindLogins { browser, url } => {
                outbox.send(sign_in::find(&self.services.plugins, &self.logins, &principal, browser, url))
            }
            ClientCommand::FillLogin { browser, url, account, item, run } => {
                let fill = sign_in::Fill { browser, url, account, item, run };
                let plugins = &self.services.plugins;
                sign_in::fill(plugins, &self.logins, self.vault.as_ref(), outbox, &principal, fill)
            }
            ClientCommand::SaveLogin { browser, account, login } => {
                let plugins = &self.services.plugins;
                outbox.send(sign_in::save(plugins, &self.logins, &principal, browser, account, &login))
            }
            ClientCommand::FindPasskeys { browser, rp_id, allowed } => {
                let plugins = &self.services.plugins;
                outbox.send(sign_in::find_passkeys(plugins, &self.logins, &principal, browser, rp_id, &allowed))
            }
            ClientCommand::SignPasskey { browser, chosen } => {
                let plugins = &self.services.plugins;
                outbox.send(sign_in::sign_passkey(plugins, &self.logins, &principal, browser, &chosen))
            }
            ClientCommand::CreatePasskey { browser, new } => {
                let plugins = &self.services.plugins;
                outbox.send(sign_in::create_passkey(plugins, &self.logins, &principal, browser, &new))
            }
            ClientCommand::InstallPlugin { plugin, folder } => {
                self.services.plugins.install_and_set_up(&principal, plugin.as_ref(), folder.as_deref(), outbox)
            }
            ClientCommand::UninstallPlugin { plugin } => {
                outbox.send(self.services.plugins.uninstall(&principal, &plugin));
                self.sync_channels(&principal);
            }
            ClientCommand::Viewing { .. } => {}
            ClientCommand::InstallLibrary { items } => {
                outbox.send(self.services.plugins.install_library(&principal, &items))
            }
            ClientCommand::ReadLibraryItem { path, name } => {
                let home = claude_home_for(&principal);
                outbox.send(match superfik_plugins::library::read_own(&name, &path, home.as_ref().map(|h| h.root())) {
                    Some((kind, body)) => ServerEvent::LibraryItemRead(superfik_core::library::LibraryItem {
                        kind,
                        name,
                        body,
                        author_name: String::new(),
                    }),
                    None => ServerEvent::error(format!("no skill or agent named {name} in this project or ~/.claude")),
                })
            }
            ClientCommand::Connect { plugin, label, values } => {
                self.services.plugins.connect(&principal, &plugin, &label, &values, outbox);
                self.sync_channels(&principal);
            }
            ClientCommand::Disconnect { connection } => {
                outbox.send(self.services.plugins.disconnect(&principal, connection));
                self.sync_channels(&principal);
            }
            ClientCommand::ListProviders => outbox.send(self.services.providers.list(&principal)),
            ClientCommand::DownloadModel { id } => {
                crate::models::download(self.services.downloads.as_ref(), id, outbox)
            }
            ClientCommand::ConnectProvider { provider, key } => {
                outbox.send(self.services.providers.connect(&principal, provider, &key))
            }
            ClientCommand::DisconnectProvider { provider } => {
                outbox.send(self.services.providers.disconnect(&principal, provider))
            }
            ClientCommand::ListChannels => self.list_channels(&principal),
            ClientCommand::ApproveDraft { connection, draft, text } => {
                self.settle_draft(&principal, connection, draft, Some(&text))
            }
            ClientCommand::DiscardDraft { connection, draft } => self.settle_draft(&principal, connection, draft, None),
        }
    }

    pub fn cancel_all(&self) {
        self.runs.cancel_all();
    }

    pub fn active_runs(&self) -> usize {
        self.runs.len()
    }

    fn submit(&self, owner: &Principal, run: RunId, request: &PromptRequest) {
        self.submit_traced(owner, run, request, None);
    }

    pub(super) fn submit_traced(
        &self,
        owner: &Principal,
        run: RunId,
        request: &PromptRequest,
        trace: Option<Arc<Trace>>,
    ) {
        if self.runs.is_active(run) {
            self.outbox.answering(owner).send(ServerEvent::error(format!("run {run} is already in progress")));
            return;
        }
        if let Err(message) = self.start(owner, run, request, trace.clone()) {
            let failed = EngineEvent::Failed(message.clone());
            if let Some(book) = trace.and_then(|trace| trace.observe(run, &failed, memory::now())) {
                self.outbox.answering(owner).send(book);
            }
            self.fail(run, message);
        }
    }

    fn start(
        &self,
        owner: &Principal,
        run: RunId,
        request: &PromptRequest,
        trace: Option<Arc<Trace>>,
    ) -> Result<(), String> {
        self.services.budget.admit(owner, request)?;
        let outbox = self.outbox.answering(owner);
        let github = self.services.github_tokens.token(owner);
        let model = if request.model.engine == superfik_core::EngineKind::Native {
            self.services
                .plugins
                .run_key(owner, &request.model, &request.connections)?
                .or_else(|| crate::native::key_for(&self.services.providers, owner, &request.model))
        } else {
            None
        };
        let request = &PromptRequest {
            credentials: superfik_core::credentials::RunCredentials {
                github,
                model,
                owner: Some(owner.clone()),
                model_guard: Some(crate::budget::model_guard(self.services.budget.clone(), owner, request)),
            },
            ..request.clone()
        };
        let engine = self.services.engines.require(request.model.engine).map_err(|error| error.to_string())?;
        let request = match &request.handoff {
            Some(handoff) => {
                let output = handoff_path(run);
                PromptRequest { handoff: Some(Handoff { kind: handoff.kind.clone(), output }), ..request.clone() }
            }
            None => request.clone(),
        };
        let vaults = self.services.plugins.vaults(owner, &request.connections)?;
        let request = self.services.plugins.brief(owner, run, &request)?;
        let request = self.open_vault(owner, run, request, vaults);
        let tool = request
            .memory
            .as_ref()
            .filter(|brief| !brief.optimizing)
            .and(self.memory_tool.as_ref())
            .map(|server| server.grant(owner, request.cwd.clone()));
        let request = self.services.memory.brief(owner, run, &request, tool);
        let request = self.open_browser(owner, run, request);
        let request = self.open_notes(owner, run, request);
        let request = self.open_channels(owner, run, request);
        let workdir = trace.as_ref().and_then(|trace| trace.workdir().map(std::path::Path::to_path_buf));
        let sink = Arc::new(RunSink {
            budget: (self.services.budget.clone(), owner.clone()),
            runs: Arc::clone(&self.runs),
            outbox,
            handoff: request.handoff.clone(),
            memory: request.memory.is_some().then(|| (self.services.memory.clone(), owner.clone(), request.clone())),
            memory_tool: self.memory_tool.clone(),
            plugins: request.plugins.clone(),
            vault: self.vault.clone(),
            browser: self.browser.clone().zip(request.browser.clone()),
            notes: self.notes_tool.clone(),
            channels: self.channel_tools.clone(),
            routine: trace,
        });
        let request = match workdir {
            Some(cwd) => PromptRequest { cwd, ..request },
            None => request,
        };
        match engine.start(run, &request, sink) {
            Ok(handle) => {
                self.runs.insert(run, owner.clone(), handle);
                Ok(())
            }
            Err(error) => {
                plugins::discard(request.plugins.as_ref());
                close_vault(self.vault.as_ref(), request.plugins.as_ref());
                close_notes(self.notes_tool.as_ref(), request.plugins.as_ref());
                close_channels(self.channel_tools.as_ref(), request.plugins.as_ref());
                if let (Some(browser), Some(tool)) = (&self.browser, &request.browser) {
                    browser.revoke(&tool.token);
                }
                if let (Some(server), Some(tool)) =
                    (&self.memory_tool, request.memory.as_ref().and_then(|m| m.tool.as_ref()))
                {
                    server.revoke(&tool.token);
                }
                Err(error.to_string())
            }
        }
    }

    fn open_vault(
        &self,
        owner: &Principal,
        run: RunId,
        request: PromptRequest,
        accounts: Vec<VaultAccount>,
    ) -> PromptRequest {
        if accounts.is_empty() {
            return request;
        }
        let Some(vault) = &self.vault else {
            tracing::warn!("vault accounts picked but the vault tools are unavailable");
            return request;
        };
        let titles: Vec<String> = accounts.iter().map(|account| account.title.clone()).collect();
        let tool = vault.grant(owner, run, request.cwd.clone(), accounts);
        let mut brief = request.plugins.clone().unwrap_or_default();
        brief.instructions.push(VaultTool::instructions(&titles));
        brief.vault = Some(tool);
        PromptRequest { plugins: Some(brief), ..request }
    }

    fn open_browser(&self, owner: &Principal, run: RunId, request: PromptRequest) -> PromptRequest {
        let tidying = request.memory.as_ref().is_some_and(|m| m.optimizing);
        let Some(browser) = self.browser.as_ref().filter(|_| request.command.is_none() && !tidying) else {
            return request;
        };
        let mut brief = request.plugins.clone().unwrap_or_default();
        brief.instructions.push(BrowserTool::instructions());
        PromptRequest { plugins: Some(brief), browser: Some(browser.grant(owner, run)), ..request }
    }

    fn open_notes(&self, owner: &Principal, run: RunId, request: PromptRequest) -> PromptRequest {
        let tidying = request.memory.as_ref().is_some_and(|m| m.optimizing);
        let Some(notes) = self.notes_tool.as_ref().filter(|_| request.command.is_none() && !tidying) else {
            return request;
        };
        let mut brief = request.plugins.clone().unwrap_or_default();
        brief.instructions.push(NotesTool::instructions());
        brief.notes = Some(notes.grant(owner, request.cwd.clone(), run));
        PromptRequest { plugins: Some(brief), ..request }
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

fn claude_home_for(principal: &Principal) -> Option<superfik_plugins::ClaudeHome> {
    match principal {
        Principal::LocalOwner => superfik_plugins::ClaudeHome::detect(),
        Principal::Member(_) => None,
    }
}

fn close_vault(vault: Option<&Vault>, plugins: Option<&PluginBrief>) {
    if let (Some(vault), Some(tool)) = (vault, plugins.and_then(|p| p.vault.as_ref())) {
        vault.revoke(&tool.token);
    }
}

fn close_channels(channels: Option<&ChannelTools>, plugins: Option<&PluginBrief>) {
    if let (Some(channels), Some(tool)) = (channels, plugins.and_then(|p| p.channels.as_ref())) {
        channels.revoke(&tool.token);
    }
}

fn close_notes(notes: Option<&NotesTools>, plugins: Option<&PluginBrief>) {
    if let (Some(notes), Some(tool)) = (notes, plugins.and_then(|p| p.notes.as_ref())) {
        notes.revoke(&tool.token);
    }
}

fn handoff_path(run: RunId) -> PathBuf {
    std::env::temp_dir().join(format!("superfik-handoff-{run}.json"))
}

struct RunSink {
    budget: (Arc<dyn crate::Budget>, Principal),
    runs: Arc<RunTable>,
    outbox: Outbox,
    handoff: Option<Handoff>,
    memory: Option<(Memories, Principal, PromptRequest)>,
    memory_tool: Option<McpServer>,
    plugins: Option<PluginBrief>,
    vault: Option<Vault>,
    browser: Option<(BrowserTools, BrowserTool)>,
    notes: Option<NotesTools>,
    channels: Option<ChannelTools>,
    routine: Option<Arc<Trace>>,
}

impl RunSink {
    fn collect_handoff(&self) -> Option<ServerEvent> {
        let handoff = self.handoff.as_ref()?;
        let json = std::fs::read_to_string(&handoff.output).ok();
        let _ = std::fs::remove_file(&handoff.output);
        let json = json?;
        match handoff.kind {
            HandoffKind::PullRequestDraft => {
                superfik_core::handoff::parse_pull_request_draft(&json).map(ServerEvent::PullRequestProposed)
            }
            HandoffKind::Review { number } => superfik_core::handoff::parse_review(&json)
                .map(|(summary, comments)| ServerEvent::ReviewDrafted { number, summary, comments }),
        }
    }
}

impl EventSink for RunSink {
    fn emit(&self, run: RunId, mut event: EngineEvent) {
        if let EngineEvent::Usage(usage) = &mut event
            && self.budget.0.authoritative()
            && let Some(cost) = usage.estimated_cost_usd
        {
            self.budget.0.spend(&self.budget.1, &usage.model, cost);
            usage.accounted_by_server = true;
        }
        if let Some(book) = self.routine.as_ref().and_then(|trace| trace.observe(run, &event, memory::now())) {
            self.outbox.send(book);
        }
        if event.is_terminal() {
            self.runs.finish(run);
            plugins::discard(self.plugins.as_ref());
            close_vault(self.vault.as_ref(), self.plugins.as_ref());
            close_notes(self.notes.as_ref(), self.plugins.as_ref());
            close_channels(self.channels.as_ref(), self.plugins.as_ref());
            if let Some((browser, tool)) = &self.browser {
                browser.revoke(&tool.token);
            }
            match self.collect_handoff() {
                Some(result) if matches!(event, EngineEvent::Completed { .. }) => self.outbox.send(result),
                _ => {}
            }
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

#[cfg(test)]
mod tests {
    use superfik_core::{OrgId, UserId};

    use super::*;

    #[test]
    fn only_the_person_at_this_computer_reads_its_claude_folder() {
        let member = Principal::member(UserId::new(), OrgId::new());
        assert!(claude_home_for(&member).is_none(), "a member must not read the server's ~/.claude");
        assert_eq!(
            claude_home_for(&Principal::LocalOwner).map(|home| home.root().to_path_buf()),
            superfik_plugins::ClaudeHome::detect().map(|home| home.root().to_path_buf())
        );
    }
}
