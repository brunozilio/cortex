//! Application state and the pure reducer that evolves it.
//!
//! `Session::apply` takes an [`Action`] and returns [`Effect`]s. It never
//! performs IO, which keeps every product rule unit-testable; the app shell
//! executes the effects (backend commands, dialogs, persistence).

mod action;
mod activity;
mod agents;
mod ask;
mod autofill;
mod browser;
mod browser_agent;
pub mod browser_profiles;
mod carousel;
pub mod channels;
mod commands;
mod comments;
mod composer;
mod connection;
mod connections;
mod context;
mod conversation;
mod delegation;
mod edit_key;
mod effect;
mod engines;
mod fill_on_load;
mod get_started;
mod github;
mod governance;
mod history;
mod library;
mod local_models;
mod lock;
pub use local_models::{LOCAL_SUGGESTIONS, hugging_face_id};
pub(crate) mod chores;
mod logins;
pub mod managed;
mod memory;
mod note_draft;
mod notes;
mod notice;
mod organization;
mod passkey;
mod passkey_create;
mod platform;
mod plugin_rows;
mod plugins;
mod pr_composer;
mod preferences;
mod presence;
mod pull_request;
mod push_to_talk;
mod review;
pub mod routines;
mod run;
mod save_login;
mod server;
pub mod server_link;
mod setting;
mod settings;
mod settings_text;
mod shared;
mod shared_memory;
mod sound;
mod steps;
mod teams;
mod timeline;
mod update;
mod upkeep;
mod visual_prompt;
mod voice;
mod voice_field;
mod voice_reply;
mod wake;
mod workspace;

pub use action::{Action, EditKey, PopoverKind};
pub use activity::ContextNode;
pub use ask::{PendingAsk, Reply};
pub use browser::{BrowserCard, BrowserFocus};
pub use carousel::{Carousel, page_count};
pub use commands::ShownNodes;
pub use notes::NoteAction;
pub use save_login::SaveOffer;
pub use superfik_core::Command;
pub use visual_prompt::VisualPrompt;

pub use connections::{Pick, SKILL};
pub use context::is_image;
pub use conversation::{ActiveRun, Conversation};
pub use effect::Effect;
pub use engines::ModelRow;
pub use github::{GitHubListing, RepositoryUse};
pub use governance::{PolicyChoice, PolicyRefusal};
pub use lock::{LockAction, LockProblem, LockState, LockStatus};
pub use logins::Logins;
pub use memory::{MemoryField, MemoryState, Revision};
pub use notice::Notice;
pub use organization::{OrgAction, OrganizationState};
pub use platform::{Platform, Primary, SystemUnlock};
pub use plugin_rows::{AccountChoice, PluginRow, account_label};
pub use plugins::{ConnectSheet, PluginsState, Shelf};
pub use pr_composer::{PrField, PullRequestComposer};
pub use preferences::{Appearance, Language, Preferences, Setting, SoundChoice, Theme};
pub use review::{DiffLayout, ReviewScreen};
pub use routines::{RoutineAction, RoutinesState};
pub use server_link::{AddressProblem, JoinRequest, Joined, ServerAction, ServerField, ServerLink};
pub use settings::{SettingsPane, SettingsScreen};
pub use settings_text::SettingsText;
pub use teams::TeamChoice;
pub use timeline::{Attention, TimelineGroup, TimelineItem, Toward};
pub use update::{UpdateAction, UpdateCall, UpdateOffer, UpdatePhase, UpdateStage, UpdateStageReport, UpdateState};
pub use voice::{VoiceAction, VoicePhase, VoiceState};
pub use voice_field::{VoiceField, VoiceFieldAction};

use std::path::PathBuf;

use superfik_core::{
    ConversationId, CustomAgent, Effort, EngineInfo, ModelOption, PullRequest, Skill, WorkspaceId, WorkspaceSnapshot,
};
use superfik_ui::TextEdit;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WallClock {
    pub unix_at_zero: i64,
    pub utc_offset_secs: i32,
}

impl WallClock {
    pub fn wall_now(self, now: f64) -> i64 {
        self.unix_at_zero + now as i64
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub workspace_id: WorkspaceId,
    pub root: Option<PathBuf>,
    pub snapshot: WorkspaceSnapshot,
    pub available_skills: Vec<Skill>,
    pub models: Vec<ModelOption>,
    pub model_changed: Option<(String, f64)>,
    pub popover: Option<PopoverKind>,
    pub model_scroll: f32,
    pub pick_cursor: usize,
    pub account_menu: Option<superfik_core::PluginId>,
    pub branch: Option<String>,
    pub notice: Option<Notice>,
    pub pull_requests: Vec<PullRequest>,
    pub pr_composer: Option<PullRequestComposer>,
    pub pending_handoff: Option<superfik_core::HandoffKind>,
    pub review: Option<ReviewScreen>,
    pub recent_workspaces: Vec<PathBuf>,
    pub engines: Vec<EngineInfo>,
    pub available_agents: Vec<CustomAgent>,
    pub conversations: Vec<Conversation>,
    pub active: ConversationId,
    pub renaming: Option<(ConversationId, TextEdit)>,
    pub collapsed_days: std::collections::HashSet<i64>,
    pub archived_open: bool,
    pub clock: WallClock,
    pub preferences: Preferences,
    pub settings: SettingsScreen,
    pub browser_profiles: browser_profiles::ProfilesState,
    pub voice_field_edit: Option<(VoiceField, superfik_ui::TextEdit)>,
    pub system_appearance: Appearance,
    pub github: Option<GitHubListing>,
    pub locating: Option<(String, RepositoryUse)>,
    pub clone_root: PathBuf,
    pub memory: MemoryState,
    pub notes: notes::NotesState,
    pub routines: RoutinesState,
    pub channels: channels::ChannelsState,
    pub nodes: ShownNodes,
    pub plugins: PluginsState,
    pub organization: OrganizationState,
    pub remote: bool,
    pub server_link: ServerLink,
    pub lock: LockState,
    pub shared_sessions: Vec<superfik_protocol::SharedSession>,
    pub shared_memory: Vec<superfik_core::memory::shared::SharedMemory>,
    pub presence: Vec<superfik_protocol::Viewer>,
    pub comments: std::collections::HashMap<superfik_core::ConversationId, Vec<superfik_core::comments::Comment>>,
    pub managed: managed::Managed,
    pub logins: Logins,
    pub last_logins: fill_on_load::LastLogins,
    pub get_started: bool,
    pub onboarded: bool,
    pub platform: Platform,
    pub voice: VoiceState,
    pub update: UpdateState,
}

impl Default for Session {
    fn default() -> Self {
        let conversation = Conversation::new(0, Effort::Medium);
        Self {
            workspace_id: WorkspaceId::new(),
            root: None,
            snapshot: WorkspaceSnapshot::default(),
            available_skills: Vec::new(),
            models: default_models(),
            model_changed: None,
            popover: None,
            model_scroll: 0.0,
            pick_cursor: 0,
            account_menu: None,
            branch: None,
            notice: None,
            pull_requests: Vec::new(),
            pr_composer: None,
            pending_handoff: None,
            review: None,
            recent_workspaces: Vec::new(),
            engines: Vec::new(),
            available_agents: Vec::new(),
            active: conversation.id,
            conversations: vec![conversation],
            renaming: None,
            collapsed_days: std::collections::HashSet::new(),
            archived_open: false,
            clock: WallClock::default(),
            preferences: Preferences::default(),
            browser_profiles: browser_profiles::ProfilesState::default(),
            voice_field_edit: None,
            settings: SettingsScreen::default(),
            system_appearance: Appearance::default(),
            github: None,
            locating: None,
            clone_root: PathBuf::new(),
            memory: MemoryState::default(),
            notes: notes::NotesState::default(),
            routines: RoutinesState::default(),
            channels: channels::ChannelsState::default(),
            nodes: ShownNodes::default(),
            plugins: PluginsState::default(),
            organization: OrganizationState::default(),
            remote: false,
            server_link: ServerLink::default(),
            lock: LockState::default(),
            shared_sessions: Vec::new(),
            shared_memory: Vec::new(),
            presence: Vec::new(),
            comments: std::collections::HashMap::new(),
            managed: managed::Managed::default(),
            logins: Logins::default(),
            last_logins: fill_on_load::LastLogins::default(),
            get_started: false,
            onboarded: false,
            platform: Platform::DESKTOP,
            voice: VoiceState::default(),
            update: UpdateState::default(),
        }
    }
}

fn default_models() -> Vec<ModelOption> {
    use superfik_core::EngineKind::Native;
    vec![
        ModelOption::new(Native, "claude-opus-5-5", "Opus 5.5"),
        ModelOption::new(Native, "claude-fable-5-1", "Fable 5.1"),
        ModelOption::new(Native, "claude-sonnet-5-5", "Sonnet 5.5"),
        ModelOption::new(Native, "claude-haiku-5-5", "Haiku 5.5"),
    ]
}

impl Session {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root, ..Self::default() }
    }

    pub fn apply(&mut self, action: Action, now: f64) -> Vec<Effect> {
        let trigger = sound::Trigger::of(&action);
        let before = sound::Probe::of(self);
        let kept = self.kept();
        let loaded = matches!(action, Action::Server(superfik_protocol::ServerEvent::Sessions { .. }));
        let on_screen = self.on_screen();
        let mut effects = self.reduce(action, now);
        effects.extend(self.keep_changed(&kept, loaded));
        effects.extend(self.report_viewing(on_screen));
        if self.platform.sound
            && let Some(cue) = sound::cue(trigger, &before, &sound::Probe::of(self))
        {
            effects.push(Effect::Sound(cue));
        }
        effects
    }

    fn reduce(&mut self, action: Action, now: f64) -> Vec<Effect> {
        match action {
            Action::BrowserProfile(action) => self.browser_profile(action),
            Action::VoiceField(action) => self.voice_field(action),
            Action::Routines(action) => self.routine(action, now),
            Action::Edit(_) if self.update.covers() => Vec::new(),
            Action::Submit if self.update.covers() => self.update_action(UpdateAction::Primary),
            Action::Update(action) => self.update_action(action),
            Action::Edit(_) if self.lock.covers() => Vec::new(),
            Action::Submit if self.lock.covers() => self.lock_action(LockAction::Unlock),
            Action::Lock(action) => self.lock_action(action),
            Action::ServerLink(action) => self.server_action(action),
            Action::Edit(_) if self.get_started => Vec::new(),
            Action::Submit if self.get_started => self.dismiss_get_started(),
            Action::ShowGetStarted => self.show_get_started(),
            Action::DismissGetStarted => self.dismiss_get_started(),
            Action::Edit(key) => self.edit(key),
            Action::Submit if self.enter_pull_request() || self.enter_review() => Vec::new(),
            Action::Submit if self.browser_focus() == Some(BrowserFocus::Address) => {
                self.enter_address().unwrap_or_default()
            }
            Action::Submit if self.memory.focus.is_some() => self.enter_memory().unwrap_or_default(),
            Action::Submit if self.notes.editing.is_some() => self.enter_note().unwrap_or_default(),
            Action::Submit if self.settings.open && self.plugins.panel.local_models => self.download_local_model(),
            Action::Submit if self.plugins.panel.sheet.is_some() => self.enter_connect_sheet().unwrap_or_default(),
            Action::Submit => self.submit(now),
            Action::Cancel => {
                let mut effects = self.cancel();
                effects.extend(self.hush_voice());
                effects
            }
            Action::Voice(action) => self.voice(action, now),
            Action::Reply(reply) => self.reply(reply),
            Action::PickChoice(index) => self.pick(index),
            Action::MoveChoice(delta) => self.move_cursor(delta),
            Action::SetPermissionMode(mode) => self.set_permission_mode(mode),
            Action::CompactContext => self.compact(),
            Action::PagePick(carousel, by) => self.page_pick(carousel, by),
            Action::RunCommand(command) => self.run_picked_command(command),
            Action::ClearResearch => self.clear_research(),
            Action::SetEffort(effort) => {
                self.conversation_mut().effort = effort;
                Vec::new()
            }
            Action::SelectModel(index) => self.select_model(index, now),
            Action::OpenPopover(kind) => {
                if kind == PopoverKind::Model && self.popover != Some(kind) {
                    self.model_scroll = 0.0;
                }
                self.toggle_popover(kind)
            }
            Action::ClosePopover => {
                let answered =
                    if self.popover == Some(PopoverKind::Passkeys) { self.passkeys_closed() } else { Vec::new() };
                self.close_popover();
                answered
            }
            Action::RemoveRepository(index) => self.remove_repository(index),
            Action::RemoveWorkspace => self.remove_workspace(),
            Action::OpenGitHubRepository(index) => self.locate_github(index, RepositoryUse::Open),
            Action::SignInToGitHub => {
                vec![Effect::Command(Box::new(superfik_protocol::ClientCommand::SignInToGitHub))]
            }
            Action::AttachGitHubRepository(index) => self.locate_github(index, RepositoryUse::Attach),
            Action::OpenReview => self.open_review(),
            Action::DelegateReview => self.delegate_review(now),
            Action::CloseReview => self.close_review(),
            Action::SelectReviewFile(index) => self.select_review_file(index),
            Action::SetDiffLayout(layout) => self.set_diff_layout(layout),
            Action::SelectDiffLine { side, line, extend } => self.select_diff_line(side, line, extend),
            Action::ScrollReview { files, diff, diff_x } => self.scroll_review(files, diff, diff_x),
            Action::AddReviewComment => self.add_review_comment(),
            Action::EditReviewComment(id) => self.edit_review_comment(id),
            Action::SaveReviewComment => self.save_review_comment(),
            Action::CancelReviewComment => self.cancel_review_comment(),
            Action::DeleteReviewComment(id) => self.delete_review_comment(id),
            Action::ResolveReviewThread { id, resolved } => self.resolve_review_thread(id, resolved),
            Action::AskAboutSelection => self.ask_about_selection(),
            Action::BeginPublishReview(verdict) => self.begin_publish_review(verdict),
            Action::ToggleOutgoingComment(id) => self.toggle_outgoing_comment(id),
            Action::CancelPublishReview => self.cancel_publish_review(),
            Action::PublishReview => self.publish_review(),
            Action::ProposePullRequest => self.propose_pull_request(),
            Action::RequestPullRequestDraft => self.request_pull_request_draft(now),
            Action::FocusPullRequestField(field) => self.focus_pull_request_field(field),
            Action::TogglePullRequestDraft => self.toggle_pull_request_draft(),
            Action::ConfirmPullRequest => self.confirm_pull_request(),
            Action::CancelPullRequest => self.cancel_pull_request(),
            Action::SelectPullRequest(index) => self.select_pull_request(index),
            Action::RemovePullRequest => self.remove_pull_request(),
            Action::RequestRepository => self.request_repository(),
            Action::AddRepositoryPath(path) => self.add_repository_path(path),
            Action::RemoveSkill(name) => self.remove_skill(&name),
            Action::AddSkill(name) => self.add_skill(&name),
            Action::Page(carousel, by) => self.flip(carousel, by),
            Action::CompleteSkill(name) => self.complete_skill(&name),
            Action::AcceptCompletion => self.accept_completion(),
            Action::MovePick(delta) => self.move_pick(delta),
            Action::AddReferences(paths) => self.add_references(paths),
            Action::RemoveReferences => self.remove_references(),
            Action::ToggleReference(index) => self.toggle_reference(index),
            Action::RemoveReference(index) => self.remove_reference(index),
            Action::RemoveAgent(index) => self.remove_agent(index),
            Action::ToggleAgent(index) => self.toggle_agent(index),
            Action::CloseAgent => self.close_agent(),
            Action::ScrollAgent { by, max } => self.scroll_agent(by, max),
            Action::SelectBranch(branch) => self.select_branch(branch),
            Action::OpenWorkspace(path) => self.open_workspace(path),
            Action::RestoreRecents(paths) => {
                self.recent_workspaces = paths;
                Vec::new()
            }
            Action::RequestWorkspace if self.remote => {
                self.popover = None;
                self.toggle_popover(PopoverKind::Repository)
            }
            Action::RequestWorkspace => vec![Effect::PickWorkspace],
            Action::RequestReferences => vec![Effect::PickReferences],
            Action::Server(event) => self.server(event, now),
            Action::ToggleSteps(turn) => self.toggle_steps(turn, now),
            Action::DismissNotice => self.dismiss_notice(),
            Action::NewConversation => self.new_conversation(),
            Action::ChooseSetting(setting) => self.choose_setting(setting),
            Action::OpenSettings => self.open_settings(),
            Action::CloseSettings => self.close_settings(),
            Action::ShowSettingsPane(pane) => self.show_settings_pane(pane),
            Action::Organization(action) => self.organization_action(action),
            Action::RestorePreferences(preferences) => {
                self.preferences = preferences;
                self.voice_restored()
            }
            Action::Managed(managed) => self.managed(managed),
            Action::SystemAppearance(appearance) => {
                self.system_appearance = appearance;
                Vec::new()
            }
            Action::SelectConversation(id) => self.select_conversation(id),
            Action::SelectShortcut(n) => self.select_shortcut(n, now),
            Action::RetryTurn(index) => self.retry_turn(index, now),
            Action::VisualPrompt(at) => self.visual_prompt(at, now),
            Action::ToggleStepOutput(index, step) => self.toggle_step_output(index, step),
            Action::StepConversation(toward) => self.step_conversation(toward),
            Action::ScrollConversation(toward) => self.scroll_conversation(toward),
            Action::ArchiveConversation(id) => self.archive_conversation(id),
            Action::UnarchiveConversation(id) => self.unarchive_conversation(id),
            Action::ToggleArchived => self.toggle_archived(),
            Action::BeginRename(id) => self.begin_rename(id),
            Action::CommitRename => self.commit_rename(),
            Action::CancelRename => {
                self.renaming = None;
                Vec::new()
            }
            Action::ToggleGroup(day) => self.toggle_group(day),
            Action::FocusMemory(field) => self.focus_memory(field),
            Action::OpenMemory => self.open_memory(),
            Action::CloseMemory => self.close_memory(),
            Action::SelectMemory(id) => self.select_memory(id),
            Action::SetMemoryKind(kind) => self.set_memory_kind(kind),
            Action::Remember => self.remember(),
            Action::ScrollMemory { by, max } => self.scroll_memory(by, max),
            Action::ForgetMemory(id) => self.forget_memory(id),
            Action::ShareMemory(id) => self.share_memory(id),
            Action::ReviseMemory(id) => self.revise_memory(id),
            Action::OptimizeMemory => self.optimize_memory(),
            Action::UndoMemory => self.undo_memory(),
            Action::Note(action) => self.note_action(action),
            Action::SetRevisionKind(kind) => self.set_revision_kind(kind),
            Action::PickConnection(id) => self.pick_connection(id),
            Action::RemoveConnection(id) => self.remove_connection(id),
            Action::UseConnection(id) => self.use_connection(id),
            Action::OpenAccountMenu(plugin) => self.open_account_menu(plugin),
            Action::ConnectPlugin(id) => self.connect_plugin(id),
            Action::OpenPlugins => self.open_plugins(),
            Action::PluginsBack => self.plugins_back(),
            Action::SelectPlugin(id) => self.select_plugin(id),
            Action::BeginAiAccount => {
                self.plugins.panel.ai_adding = true;
                Vec::new()
            }
            Action::ToggleLocalModels => self.toggle_local_models(),
            Action::FocusLocalModel => {
                self.plugins.panel.local_focus = 0;
                Vec::new()
            }
            Action::ToggleLocalModelPicker => {
                self.plugins.panel.local_picker = !self.plugins.panel.local_picker;
                Vec::new()
            }
            Action::ChooseLocalModel(index) => self.choose_local_model(index, now),
            Action::DownloadLocalModel => self.download_local_model(),
            Action::InstallPlugin(id) => self.install_plugin(id),
            Action::UninstallPlugin(id) => self.uninstall_plugin(id),
            Action::RequestPluginFolder => vec![Effect::PickPluginFolder],
            Action::InstallPluginFolder(folder) => self.install_plugin_folder(folder),
            Action::BeginConnect(id) => self.begin_connect(id),
            Action::FocusConnectField(index) => self.focus_connect_field(index),
            Action::StepConnectFocus { back } => self.step_connect_focus(back),
            Action::ConfirmConnect => self.confirm_connect(),
            Action::OpenBrowser => self.open_browser(),
            Action::CloseBrowser => self.close_browser(),
            Action::FocusBrowser(focus) => self.focus_browser(focus),
            Action::BrowserBack => self.browse(superfik_browser::Call::Back),
            Action::BrowserForward => self.browse(superfik_browser::Call::Forward),
            Action::Reload => self.browse(superfik_browser::Call::Reload),
            Action::BrowserPage(id, event) => self.page_event(id, event),
            Action::TogglePrivateBrowsing => self.toggle_private(),
            Action::ToggleBrowserExpanded => self.toggle_browser_expanded(),
            Action::BrowserSources(sources) => {
                self.logins.sources = sources;
                Vec::new()
            }
            Action::ImportLogins(kind) => self.import_logins(kind, now),
            Action::FillLogin(index) => self.fill_login(index),
            Action::SaveLogin(index) => self.save_login(index),
            Action::DismissSaveLogin => self.dismiss_save_login(),
            Action::UsePasskey(index) => self.use_passkey(index),
            Action::PasskeyElsewhere => self.passkey_elsewhere(),
            Action::LoginsImported(kind, result) => self.logins_imported(kind, result, now),
            Action::ClearBrowserCookies => self.clear_browser_cookies(now),
            Action::Disconnect(id) => self.disconnect(id),
            Action::CopyChannelHook(id) => self.copy_channel_hook(id),
            Action::ScrollSettings { by, max } => self.scroll_settings(by, max),
            Action::ScrollModels { by, max } => {
                if self.popover == Some(PopoverKind::Model) {
                    self.model_scroll = (self.model_scroll + by).clamp(0.0, max.max(0.0));
                }
                Vec::new()
            }
        }
    }

    pub fn conversation(&self) -> &Conversation {
        self.conversations.iter().find(|c| c.id == self.active).unwrap_or(&self.conversations[0])
    }

    pub fn conversation_mut(&mut self) -> &mut Conversation {
        let index = self.active_index();
        &mut self.conversations[index]
    }

    pub fn launching() -> Self {
        Self { models: Vec::new(), ..Self::default() }
    }

    pub fn model(&self) -> &ModelOption {
        self.model_of(self.active_index())
    }

    pub fn model_of(&self, index: usize) -> &ModelOption {
        static NONE: std::sync::LazyLock<ModelOption> =
            std::sync::LazyLock::new(|| ModelOption::new(superfik_core::EngineKind::Native, "", ""));
        let chosen = self.conversations.get(index).map_or(0, |c| c.model);
        self.models.get(chosen).or(self.models.last()).unwrap_or(&NONE)
    }

    pub fn has_models(&self) -> bool {
        !self.models.is_empty()
    }

    pub fn effort(&self) -> Effort {
        self.conversation().effort
    }

    pub fn is_running(&self) -> bool {
        self.conversation().is_running()
    }

    pub fn any_running(&self) -> bool {
        self.conversations.iter().any(Conversation::is_running)
    }

    pub fn is_node_active(&self, node: ContextNode, now: f64) -> bool {
        self.conversation().is_node_active(node, now)
    }

    pub fn is_gathering(&self) -> bool {
        self.conversation().is_gathering()
    }

    pub fn selected_skills(&self) -> Vec<Skill> {
        self.conversation().selected_skills(&self.available_skills)
    }

    fn new_conversation(&mut self) -> Vec<Effect> {
        self.popover = None;
        self.renaming = None;
        let conversation = Conversation::new(self.conversation().model, self.conversation().effort);
        self.active = conversation.id;
        self.conversations.push(conversation);
        self.prune();
        Vec::new()
    }

    fn select_conversation(&mut self, id: ConversationId) -> Vec<Effect> {
        self.open_shared(id);
        if self.conversations.iter().any(|c| c.id == id && !c.archived) {
            if let Some(conversation) = self.conversations.iter_mut().find(|c| c.id == id) {
                conversation.ended_unseen = false;
            }
            self.active = id;
            self.popover = None;
            self.renaming = None;
            self.prune();
            return self.recap_on_return(self.active_index());
        }
        Vec::new()
    }

    fn archive_conversation(&mut self, id: ConversationId) -> Vec<Effect> {
        let Some(conversation) = self.conversations.iter_mut().find(|c| c.id == id) else { return Vec::new() };
        conversation.archived = true;
        if self.renaming.as_ref().is_some_and(|(renaming, _)| *renaming == id) {
            self.renaming = None;
        }
        if self.active == id {
            let newest = self.conversations.iter().filter(|c| !c.archived).max_by_key(|c| c.started_unix).map(|c| c.id);
            match newest {
                Some(next) => self.active = next,
                None => {
                    let fresh = Conversation::new(self.conversation().model, self.conversation().effort);
                    self.active = fresh.id;
                    self.conversations.push(fresh);
                }
            }
        }
        self.prune();
        Vec::new()
    }

    fn unarchive_conversation(&mut self, id: ConversationId) -> Vec<Effect> {
        if let Some(conversation) = self.conversations.iter_mut().find(|c| c.id == id && c.archived) {
            conversation.archived = false;
        }
        Vec::new()
    }

    fn prune(&mut self) {
        let active = self.active;
        self.conversations.retain(|c| c.id == active || !c.is_disposable());
    }

    fn begin_rename(&mut self, id: ConversationId) -> Vec<Effect> {
        if let Some(conversation) = self.conversations.iter().find(|c| c.id == id && !c.archived) {
            let title = conversation.title.clone().unwrap_or_else(|| self.strings().t(superfik_i18n::Text::NewSession));
            let mut edit = TextEdit::new(title);
            edit.select_all();
            self.renaming = Some((id, edit));
            self.popover = None;
        }
        Vec::new()
    }

    fn commit_rename(&mut self) -> Vec<Effect> {
        if let Some((id, edit)) = self.renaming.take() {
            let title = edit.text().trim().to_owned();
            if let Some(conversation) = self.conversations.iter_mut().find(|c| c.id == id)
                && !title.is_empty()
            {
                conversation.title = Some(title);
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_ask;
#[cfg(test)]
mod tests_autofill;
#[cfg(test)]
mod tests_browser;
#[cfg(test)]
mod tests_chores;
#[cfg(test)]
mod tests_commands;
#[cfg(test)]
mod tests_completion;
#[cfg(test)]
mod tests_conversations;
#[cfg(test)]
mod tests_engine;
#[cfg(test)]
mod tests_memory;
#[cfg(test)]
mod tests_notes;
#[cfg(test)]
mod tests_passkey;
#[cfg(test)]
mod tests_plugin_rows;
#[cfg(test)]
mod tests_plugin_sheet;
#[cfg(test)]
mod tests_plugins;
#[cfg(test)]
mod tests_push_to_talk;
#[cfg(test)]
mod tests_save_login;
#[cfg(test)]
mod tests_upkeep;
#[cfg(test)]
mod tests_voice;
#[cfg(test)]
mod tests_voice_field;
#[cfg(test)]
mod tests_wake;
