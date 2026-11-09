//! Folding backend events into the session.

use superfik_core::{EngineEvent, EngineProblem, RunId, SessionCommand};
use superfik_protocol::ServerEvent;

use super::{Effect, Session};

impl Session {
    pub(super) fn server(&mut self, event: ServerEvent, now: f64) -> Vec<Effect> {
        match event {
            ServerEvent::Run { run, event } => {
                let handed_over = self.handed_over();
                let mut effects = match &event {
                    EngineEvent::Usage(usage) if !usage.accounted_by_server => {
                        self.report_usage(run, usage.estimated_cost_usd)
                    }
                    EngineEvent::Completed { cost_usd, .. }
                        if self
                            .conversations
                            .iter()
                            .find(|c| c.owns_run(run))
                            .and_then(|c| self.models.get(c.model))
                            .is_none_or(|m| m.engine != superfik_core::EngineKind::Native) =>
                    {
                        self.report_usage(run, *cost_usd)
                    }
                    _ => Vec::new(),
                };
                effects.extend(self.run_event(run, &event, now));
                effects.extend(self.release_handed_over(handed_over));
                return effects;
            }
            ServerEvent::Page { browser, report } => return self.page_event(browser, report.into()),
            ServerEvent::PageFrame(_) => {}
            ServerEvent::Providers(superfik_protocol::ProviderEvent::Failed { provider, message }) => {
                self.notice = Some(super::Notice::Raw(format!("{}: {message}", provider.name())));
            }
            ServerEvent::Providers(superfik_protocol::ProviderEvent::Download { id, state }) => {
                return self.local_download_event(id, state);
            }
            ServerEvent::Providers(_) => {}
            ServerEvent::Delegated(waiting) => self.delegated_listed(waiting),
            ServerEvent::Library(items) => return self.library_listed(items),
            ServerEvent::Presence(viewers) => self.presence_arrived(viewers),
            ServerEvent::Comments { session, comments } => self.comments_arrived(session, comments),
            ServerEvent::Teams(teams) => {
                if self.organization.team_open.is_some_and(|open| !teams.iter().any(|team| team.id == open)) {
                    self.organization.team_open = None;
                }
                self.organization.teams = teams;
            }
            ServerEvent::Policy(policy) => return self.policy_arrived(policy),
            ServerEvent::Spent { usd, .. } => return self.spent_arrived(usd),
            ServerEvent::Usage(usage) => self.usage_arrived(usage),
            ServerEvent::LibraryItemRead(item) => return self.library_item_read(item),
            ServerEvent::SharedMemory { project, memories } => self.shared_memory_arrived(&project, memories),
            ServerEvent::DelegatedAnswered { run, ask, decision, by } => {
                return self.delegated_answered(run, &ask, decision, by);
            }
            ServerEvent::Models(models) => self.replace_models(models),
            ServerEvent::Welcome { principal, .. } => return self.welcomed(principal),
            ServerEvent::Members(members) => return self.members_arrived(members),
            ServerEvent::FirstAccessCode { name, code, .. } => self.first_access_code(name, code),
            ServerEvent::ServerSettings(settings) => self.server_settings_arrived(settings),
            ServerEvent::Workspace(snapshot) => {
                self.branch = snapshot.repository.as_ref().map(|r| r.branch.clone());
                self.snapshot = snapshot;
                self.pull_request_arrived();
                self.adopt_branch_pull_request();
                return self.load_project_sessions();
            }
            ServerEvent::SharedSessions { project, sessions } => self.shared_sessions_arrived(&project, sessions),
            ServerEvent::PullRequests(pull_requests) => self.pull_requests = pull_requests,
            ServerEvent::PullRequestCreated(pull_request) => self.pull_request_created(pull_request),
            ServerEvent::PullRequestProposed(draft) => self.pull_request_proposed(draft),
            ServerEvent::PullRequestLoaded(detail) => self.pull_request_loaded(*detail),
            ServerEvent::ReviewSubmitted { number } => return self.review_submitted(number),
            ServerEvent::ReviewDrafted { number, comments, .. } => self.review_drafted(number, comments),
            ServerEvent::ThreadResolved { thread, resolved } => self.thread_resolved(&thread, resolved),
            ServerEvent::Repository(repository) => {
                self.add_repository(repository);
            }
            ServerEvent::GitHubRepositories { signed_in, repositories } => {
                self.github = Some(super::GitHubListing { signed_in, repositories });
            }
            ServerEvent::RepositoryLocated { slug, path } => return self.repository_located(&slug, path),
            ServerEvent::Skills(skills) => self.available_skills = skills,
            ServerEvent::Engines(engines) => self.set_engines(engines),
            ServerEvent::Agents(agents) => self.set_available_agents(agents),
            ServerEvent::Memory { path, memories, learned } => {
                return self.memory_arrived(&path, memories, learned, now);
            }
            ServerEvent::MemorySearch { path, scores, .. } => self.memory_scored(&path, &scores),
            ServerEvent::Notes { path, notes, by, created, .. } => self.notes_arrived(&path, notes, by, &created),
            ServerEvent::Sessions { path, sessions } => self.sessions_arrived(&path, sessions),
            ServerEvent::Routines { path, book } => self.routines_arrived(path, book),
            ServerEvent::RoutineWebhook { path, routine, hook, token } => {
                return self.webhook_arrived(path, routine, hook, token, now);
            }
            ServerEvent::Channel { connection, status, drafts } => self.channel_arrived(connection, status, drafts),
            event @ (ServerEvent::LoginCandidates { .. }
            | ServerEvent::LoginSecret { .. }
            | ServerEvent::LoginFailed { .. }
            | ServerEvent::LoginSaved { .. }) => return self.logins_event(event),
            event @ (ServerEvent::PasskeyCandidates { .. }
            | ServerEvent::PasskeySigned { .. }
            | ServerEvent::PasskeyCreated { .. }
            | ServerEvent::PasskeyFailed { .. }) => return self.passkeys_event(event),
            event @ (ServerEvent::Plugins { .. }
            | ServerEvent::AuthorizeAt { .. }
            | ServerEvent::PluginFailed { .. }) => return self.plugins_event(event),
            ServerEvent::Link(state) => self.linked(state),
            ServerEvent::Settings { .. } => {}
            ServerEvent::Uploaded { from, path } => return self.uploaded(from, path),
            ServerEvent::GitHubCode { code, url } => {
                self.notice = Some(super::Notice::GitHubCode(code));
                return vec![Effect::OpenUrl(url)];
            }
            ServerEvent::GitHubSignedIn { .. } => {
                if matches!(self.notice, Some(super::Notice::GitHubCode(_))) {
                    self.notice = None;
                }
                return vec![Effect::Command(Box::new(superfik_protocol::ClientCommand::ListGitHubRepositories))];
            }
            ServerEvent::Error { message } => {
                self.locating = None;
                self.pull_request_failed();
                self.review_failed();
                if matches!(self.notice, Some(super::Notice::LoadingPullRequest(_))) {
                    self.notice = None;
                }
                self.notice = Some(super::Notice::Raw(message));
            }
        }
        Vec::new()
    }

    fn run_event(&mut self, run: RunId, event: &EngineEvent, now: f64) -> Vec<Effect> {
        if self.optimization_event(run, event) {
            return Vec::new();
        }
        if let Some(index) = self.conversations.iter().position(|c| c.chores.owns(run)) {
            return self.chore_event(index, run, event);
        }
        if let Some(index) = self.conversations.iter().position(|c| c.owns_upkeep(run)) {
            return self.upkeep_event(index, event);
        }
        let Some(index) = self.conversations.iter().position(|c| c.owns_run(run)) else {
            if let EngineEvent::Browse { id, .. } = event {
                let answer = Err("This run has no conversation on screen.".to_owned());
                let command = superfik_protocol::ClientCommand::AnswerBrowse { run, id: id.clone(), answer };
                return vec![Effect::Command(Box::new(command))];
            }
            return Vec::new();
        };
        self.conversations[index].apply_run_event(event, &self.available_agents, now);
        if event.is_terminal() {
            self.release_browser(index);
            if self.conversations[index].id != self.active {
                self.conversations[index].ended_unseen = true;
            }
        }
        let mut effects = self.voice_run_event(self.conversations[index].id, event);
        match event {
            EngineEvent::Browse { id, op } => effects.extend(self.agent_browse(index, run, id.clone(), op.clone())),
            EngineEvent::SessionStarted(_) => effects.extend(self.ask_title(index)),
            EngineEvent::Diagnosed(problem) => self.diagnosed(index, *problem),
            EngineEvent::Failed(message) => {
                self.notice.get_or_insert_with(|| super::Notice::Raw(message.clone()));
            }
            EngineEvent::Completed { .. } => {
                effects.extend(self.start_upkeep(index, SessionCommand::MeasureContext));
                effects.extend(self.suggest_next(index));
                effects.extend(self.look_for_heads_up(index));
                if let Some(path) = self.workspace_root() {
                    effects
                        .push(Effect::Command(Box::new(superfik_protocol::ClientCommand::InspectWorkspace { path })));
                }
            }
            _ => {}
        }
        effects
    }

    pub(super) fn diagnosed(&mut self, index: usize, problem: EngineProblem) {
        let engine = self.model_of(index).engine;
        if problem == EngineProblem::SessionNotFound {
            self.conversations[index].forget_session();
        }
        self.notice = Some(super::Notice::Engine(problem, engine));
    }
}
