//! What a click on each target means. Pure and tested.

use superfik_ui::components::{BrowserPart, ComposerPart};

use crate::input::keys::KeyCommand;
use crate::state::MemoryField;
use crate::state::{
    Action, BrowserFocus, EditKey, LockAction, OrgAction, PluginRow, PopoverKind, Reply, RoutineAction, Session,
    UpdateAction, VoiceFieldAction,
};
use crate::view::layout::NodeKey;
use crate::view::target::{
    AskTarget, GetStartedTarget, LockTarget, MemoryTarget, PluginsTarget, ReviewTarget, RoutinesTarget, SettingsTarget,
    Target, UpdateTarget,
};
use crate::view::{popovers, settings_screen};

pub fn click(target: Option<Target>, session: &Session) -> Vec<Action> {
    let mut actions = Vec::new();
    let keeps_popover = matches!(
        target,
        Some(
            Target::ModelItem(_)
                | Target::BranchItem(_)
                | Target::AttachRepositoryItem(_)
                | Target::GitHubRepositoryItem(_)
                | Target::AttachGitHubRepositoryItem(_)
                | Target::PullRequestItem(_)
                | Target::SkillItem(_)
                | Target::Command(_)
                | Target::AddSkillItem(_)
                | Target::RepositoryItem(_)
                | Target::PermissionItem(_)
                | Target::Input
        )
    ) || matches!(target, Some(Target::Surface | Target::Node(_))) && session.popover.is_some();
    if session.popover.is_some() && !keeps_popover && !opens_popover(target) {
        actions.push(Action::ClosePopover);
    }
    if let Some((renaming, _)) = &session.renaming {
        if target == Some(Target::Conversation(*renaming)) {
            return actions;
        }
        actions.push(Action::CommitRename);
    }
    let keeps_memory_focus = session.memory.open
        || matches!(target, Some(Target::Memory(MemoryTarget::Search | MemoryTarget::Draft | MemoryTarget::Kind(_))));
    if session.memory.focus.is_some() && !keeps_memory_focus {
        actions.push(Action::FocusMemory(None));
    }
    let keeps_browser_focus = matches!(target, Some(Target::Browser(BrowserPart::Address | BrowserPart::Page)));
    if session.browser_focus().is_some() && !keeps_browser_focus {
        actions.push(Action::FocusBrowser(None));
    }
    actions.extend(super::note_input::leave_draft(session, target));
    let Some(target) = target else { return actions };
    actions.extend(match target {
        Target::Note(note) => super::note_input::click(note),
        Target::Memory(memory) => memory_click(memory, session),
        Target::Node(NodeKey::Memory) => vec![Action::OpenMemory],
        Target::Composer(part) => composer(part, session),
        Target::AddRepository if session.snapshot.repository.is_none() && session.root.is_none() => {
            if session.recent_workspaces.is_empty() && session.github.is_none() {
                vec![Action::RequestWorkspace]
            } else {
                vec![Action::OpenPopover(PopoverKind::Repository)]
            }
        }
        Target::RemoveWorkspace => vec![Action::RemoveWorkspace],
        Target::RepositoryItem(i) => {
            session.recent_choices().get(i).map(|p| Action::OpenWorkspace(p.to_path_buf())).into_iter().collect()
        }
        Target::ChooseRepository => vec![Action::RequestWorkspace],
        Target::GitHubRepositoryItem(i) => vec![Action::OpenGitHubRepository(i)],
        Target::SignInToGitHub => vec![Action::SignInToGitHub],
        Target::AttachGitHubRepositoryItem(i) => vec![Action::AttachGitHubRepository(i)],
        Target::Branch => vec![Action::OpenPopover(PopoverKind::Branch)],
        Target::PullRequests => vec![Action::OpenPopover(PopoverKind::PullRequests)],
        Target::OpenPullRequest => vec![Action::ProposePullRequest],
        Target::OpenReview => vec![Action::OpenReview],
        Target::Close(NodeKey::PullRequest) => vec![Action::RemovePullRequest],
        Target::DelegateReview => vec![Action::DelegateReview],
        Target::Review(review) => review_click(review),
        Target::OpenPlugins => vec![Action::OpenPlugins],
        Target::PullRequestField(field) => vec![Action::FocusPullRequestField(field)],
        Target::WritePullRequestWithAgent => vec![Action::RequestPullRequestDraft],
        Target::TogglePullRequestDraft => vec![Action::TogglePullRequestDraft],
        Target::ConfirmPullRequest => vec![Action::ConfirmPullRequest],
        Target::CancelPullRequest => vec![Action::CancelPullRequest],
        Target::PullRequestItem(i) => vec![Action::SelectPullRequest(i)],
        Target::BranchItem(i) => {
            popovers::branches(session).get(i).map(|b| Action::SelectBranch(b.clone())).into_iter().collect()
        }
        Target::AddRepository if session.attachable_repositories().is_empty() && session.github.is_none() => {
            vec![Action::RequestRepository]
        }
        Target::AddRepository => vec![Action::OpenPopover(PopoverKind::AttachRepository)],
        Target::AttachRepositoryItem(i) => {
            session.attachable_repositories().get(i).map(|p| Action::AddRepositoryPath(p.clone())).into_iter().collect()
        }
        Target::ChooseAttachedRepository => vec![Action::RequestRepository],
        Target::RemoveRepository(i) => vec![Action::RemoveRepository(i)],
        Target::ModelItem(i) => vec![Action::SelectModel(i)],
        Target::Command(i) => superfik_core::Command::ALL.get(i).map(|c| Action::RunCommand(*c)).into_iter().collect(),
        Target::SkillItem(i) => session.picks().get(i).map(|pick| pick.action()).into_iter().collect(),
        Target::PluginRow(i) => session.plugin_rows().get(i).map(PluginRow::action).into_iter().collect(),
        Target::AccountChoice(i) => session.account_choice_action(i).into_iter().collect(),
        Target::AddReference => vec![Action::RequestReferences],
        Target::Reference(i) => vec![Action::ToggleReference(i)],
        Target::Scrim | Target::CloseViewer => close_viewer(session).into_iter().collect(),
        Target::RemoveReference(i) => vec![Action::RemoveReference(i)],
        Target::AddSkill => vec![Action::OpenPopover(PopoverKind::AddSkill)],
        Target::Page(carousel, by) if session.picking().is_some() => vec![Action::PagePick(carousel, by)],
        Target::Page(carousel, by) => vec![Action::Page(carousel, by)],
        Target::AddSkillItem(i) => {
            session.addable_skills().get(i).map(|s| Action::AddSkill(s.name.clone())).into_iter().collect()
        }
        Target::RemoveSkill(i) => {
            session.selected_skills().get(i).map(|s| Action::RemoveSkill(s.name.clone())).into_iter().collect()
        }
        Target::Close(NodeKey::References) => vec![Action::RemoveReferences],
        Target::Close(NodeKey::Agent(i)) => vec![Action::RemoveAgent(i)],
        Target::Agent(i) => vec![Action::ToggleAgent(i)],
        Target::CloseAgent => vec![Action::CloseAgent],
        Target::Close(NodeKey::Research) => vec![Action::ClearResearch],
        Target::Close(NodeKey::Browser) => vec![Action::CloseBrowser],
        Target::Browser(part) => vec![match part {
            BrowserPart::Back => Action::BrowserBack,
            BrowserPart::Forward => Action::BrowserForward,
            BrowserPart::Reload => Action::Reload,
            BrowserPart::Address => Action::FocusBrowser(Some(BrowserFocus::Address)),
            BrowserPart::Page => Action::FocusBrowser(Some(BrowserFocus::Page)),
            BrowserPart::Logins if session.browser().is_some_and(|b| b.passkey_asks()) => {
                Action::OpenPopover(PopoverKind::Passkeys)
            }
            BrowserPart::Logins if session.browser().is_some_and(|b| b.offer.is_some()) => {
                Action::OpenPopover(PopoverKind::SaveLogin)
            }
            BrowserPart::Logins if session.browser().is_some_and(|b| b.expanded) => Action::FillLogin(0),
            BrowserPart::Logins => Action::OpenPopover(PopoverKind::Logins),
            BrowserPart::Private => Action::TogglePrivateBrowsing,
            BrowserPart::Expand | BrowserPart::Header => Action::ToggleBrowserExpanded,
        }],
        Target::CompactContext => vec![Action::CompactContext],
        Target::Steps(turn) => vec![Action::ToggleSteps(turn)],
        Target::RetryTurn(turn) => vec![Action::RetryTurn(turn)],
        Target::VisualPrompt(at) => vec![Action::VisualPrompt(at)],
        Target::StepOutput(turn, step) => vec![Action::ToggleStepOutput(turn, step)],
        Target::RecentreComposer => Vec::new(),
        Target::DismissNotice => vec![Action::DismissNotice],
        Target::Conversation(id) => vec![Action::SelectConversation(id)],
        Target::NewConversation => vec![Action::NewConversation],
        Target::ArchiveConversation(id) => vec![Action::ArchiveConversation(id)],
        Target::DayGroup(day) => vec![Action::ToggleGroup(day)],
        Target::ArchivedGroup => vec![Action::ToggleArchived],
        Target::ArchivedConversation(id) => vec![Action::UnarchiveConversation(id), Action::SelectConversation(id)],
        Target::UnarchiveConversation(id) => vec![Action::UnarchiveConversation(id)],
        Target::OpenSettings => vec![Action::OpenSettings],
        Target::Settings(target) => settings_click(target, session),
        Target::OpenRoutines if session.routines.open => vec![Action::Routines(RoutineAction::Close)],
        Target::OpenRoutines => vec![Action::Routines(RoutineAction::Open)],
        Target::Routines(target) => routines_click(target).map(Action::Routines).into_iter().collect(),
        Target::Ask(AskTarget::Option(i)) => vec![Action::PickChoice(i)],
        Target::Ask(AskTarget::Reply(reply)) => vec![Action::Reply(reply)],
        Target::PermissionItem(mode) => vec![Action::SetPermissionMode(mode)],
        Target::LoginItem(index) => vec![Action::FillLogin(index)],
        Target::SaveLoginItem(index) => vec![Action::SaveLogin(index)],
        Target::DismissSaveLogin => vec![Action::DismissSaveLogin],
        Target::PasskeyItem(index) => vec![Action::UsePasskey(index)],
        Target::PasskeyElsewhere => vec![Action::PasskeyElsewhere],
        Target::GetStarted(GetStartedTarget::Continue) => vec![Action::DismissGetStarted],
        Target::GetStarted(GetStartedTarget::Surface) => Vec::new(),
        Target::Lock(LockTarget::Unlock) => vec![Action::Lock(LockAction::Unlock)],
        Target::Lock(LockTarget::Surface) => Vec::new(),
        Target::Update(target) => vec![Action::Update(match target {
            UpdateTarget::Primary => UpdateAction::Primary,
            UpdateTarget::Close => UpdateAction::Close,
            UpdateTarget::Open => UpdateAction::Open,
            UpdateTarget::Surface => return Vec::new(),
        })],
        Target::Close(
            NodeKey::Repository
            | NodeKey::Skills
            | NodeKey::Context
            | NodeKey::Memory
            | NodeKey::Notes
            | NodeKey::Connections
            | NodeKey::Commands,
        )
        | Target::Input
        | Target::Surface
        | Target::Node(_) => Vec::new(),
    });
    actions
}

fn opens_popover(target: Option<Target>) -> bool {
    matches!(
        target,
        Some(
            Target::Branch
                | Target::PullRequests
                | Target::AddRepository
                | Target::AddSkill
                | Target::Composer(ComposerPart::Model | ComposerPart::Permission)
                | Target::Browser(BrowserPart::Logins)
        )
    )
}

fn composer(part: ComposerPart, session: &Session) -> Vec<Action> {
    match part {
        ComposerPart::Model => vec![Action::OpenPopover(PopoverKind::Model)],
        ComposerPart::Effort => Vec::new(),
        ComposerPart::Image => vec![Action::RequestReferences],
        ComposerPart::Permission => vec![Action::OpenPopover(PopoverKind::Permission)],
        ComposerPart::Send if session.is_running() => vec![Action::Cancel],
        ComposerPart::Send => vec![Action::Submit],
        ComposerPart::Mic if session.offers_voice() => vec![Action::Voice(crate::state::VoiceAction::Toggle)],
        ComposerPart::Mic => Vec::new(),
    }
}

fn memory_click(target: MemoryTarget, session: &Session) -> Vec<Action> {
    match target {
        MemoryTarget::Search => vec![Action::FocusMemory(Some(MemoryField::Search))],
        MemoryTarget::Open => vec![Action::OpenMemory],
        MemoryTarget::Item(id) if !session.memory.open => vec![Action::OpenMemory, Action::SelectMemory(id)],
        MemoryTarget::Item(id) => vec![Action::SelectMemory(id)],
        MemoryTarget::Forget(id) => vec![Action::ForgetMemory(id)],
        MemoryTarget::Share(id) => vec![Action::ShareMemory(id)],
        MemoryTarget::Revise(id) => vec![Action::ReviseMemory(id)],
        MemoryTarget::Revision => vec![Action::FocusMemory(Some(MemoryField::Revision))],
        MemoryTarget::RevisionKind(kind) => vec![Action::SetRevisionKind(kind)],
        MemoryTarget::Kind(kind) => vec![Action::SetMemoryKind(kind)],
        MemoryTarget::Draft => vec![Action::FocusMemory(Some(MemoryField::Draft))],
        MemoryTarget::Remember => vec![Action::Remember],
        MemoryTarget::Close => vec![Action::FocusMemory(None), Action::CloseMemory],
        MemoryTarget::Optimize => vec![Action::OptimizeMemory],
        MemoryTarget::Undo => vec![Action::UndoMemory],
        MemoryTarget::Graph => session.memory.selected.map(Action::SelectMemory).into_iter().collect(),
        MemoryTarget::Surface => Vec::new(),
    }
}

pub fn ask_key(session: &Session, command: &KeyCommand) -> Option<Action> {
    let busy = session.renaming.is_some()
        || session.pr_composer.is_some()
        || session.popover.is_some()
        || session.memory.open
        || session.memory.focus.is_some()
        || session.browser_focus().is_some()
        || session.review.as_ref().is_some_and(|r| r.open)
        || close_viewer(session).is_some();
    if busy {
        return None;
    }
    session.pending_ask()?;
    Some(match command {
        KeyCommand::Submit => Action::Reply(Reply::Allow),
        KeyCommand::SubmitAlways => Action::Reply(Reply::AllowAlways),
        KeyCommand::Escape => Action::Reply(Reply::Deny),
        KeyCommand::Edit(EditKey::Home { .. } | EditKey::Up { .. }) => Action::MoveChoice(-1),
        KeyCommand::Edit(EditKey::End { .. } | EditKey::Down { .. }) => Action::MoveChoice(1),
        KeyCommand::Edit(EditKey::Insert(text)) => match text.parse::<usize>() {
            Ok(digit @ 1..=9) => Action::PickChoice(digit - 1),
            _ => return None,
        },
        _ => return None,
    })
}

pub fn app_owns_key(command: &KeyCommand) -> bool {
    matches!(
        command,
        KeyCommand::OpenBrowser
            | KeyCommand::Escape
            | KeyCommand::Reload
            | KeyCommand::Back
            | KeyCommand::Forward
            | KeyCommand::FillLogin
            | KeyCommand::NewConversation
            | KeyCommand::SelectSession(_)
            | KeyCommand::OpenWorkspace
            | KeyCommand::Settings
            | KeyCommand::StepSession(_)
            | KeyCommand::ZoomIn
            | KeyCommand::ZoomOut
            | KeyCommand::ResetView
    )
}

pub fn picker_key(session: &Session, command: &KeyCommand) -> Option<Action> {
    if matches!(command, KeyCommand::Tab { back: false }) && session.completion().is_some() {
        return Some(Action::AcceptCompletion);
    }
    if session.renaming.is_some() || session.popover != Some(PopoverKind::Skills) || session.picks().is_empty() {
        return None;
    }
    match command {
        KeyCommand::Edit(EditKey::Home { select: false } | EditKey::Up { select: false }) => Some(Action::MovePick(-1)),
        KeyCommand::Edit(EditKey::End { select: false } | EditKey::Down { select: false }) => Some(Action::MovePick(1)),
        KeyCommand::Edit(EditKey::Left { select: false }) => page_key(session, -1),
        KeyCommand::Edit(EditKey::Right { select: false }) => page_key(session, 1),
        _ => None,
    }
}

fn page_key(session: &Session, by: i32) -> Option<Action> {
    let carousel = session.highlighted_pick()?.carousel();
    (session.pick_pages(carousel) > 1).then_some(Action::PagePick(carousel, by))
}

pub fn enter(session: &Session) -> Action {
    if session.renaming.is_some() {
        return Action::CommitRename;
    }
    match (session.popover, session.highlighted_pick()) {
        (Some(PopoverKind::Skills), Some(pick)) => pick.action(),
        _ => Action::Submit,
    }
}

pub fn escape(session: &Session) -> Action {
    if session.update.covers() {
        return Action::Update(crate::state::UpdateAction::Close);
    }
    if session.lock.covers() {
        return Action::DismissNotice;
    }
    if session.get_started {
        return Action::DismissGetStarted;
    }
    if session.settings.open {
        return Action::PluginsBack;
    }
    if session.routines.open {
        return Action::Routines(RoutineAction::Back);
    }
    if session.renaming.is_some() {
        return Action::CancelRename;
    }
    if session.pr_composer.is_some() {
        return Action::CancelPullRequest;
    }
    if session.conversation().open_agent.is_some() {
        return Action::CloseAgent;
    }
    if session.memory.open || session.memory.focus.is_some() {
        return Action::CloseMemory;
    }
    if session.notes.editing.is_some() {
        return Action::Note(crate::state::NoteAction::Close);
    }
    if session.browser_focus() == Some(BrowserFocus::Address) {
        return Action::FocusBrowser(None);
    }
    if session.browser().is_some_and(|card| card.expanded) {
        return Action::ToggleBrowserExpanded;
    }
    if session.browser_focus() == Some(BrowserFocus::Page) {
        return Action::FocusBrowser(None);
    }
    if let Some(review) = session.review.as_ref().filter(|r| r.open) {
        return if review.publish.is_some() {
            Action::CancelPublishReview
        } else if review.editor.is_some() {
            Action::CancelReviewComment
        } else {
            Action::CloseReview
        };
    }
    if let Some(close) = close_viewer(session) {
        return close;
    }
    if session.popover.is_none() && session.is_running() {
        return Action::Cancel;
    }
    Action::ClosePopover
}

fn organization_click(run: settings_screen::rows::Run) -> Option<OrgAction> {
    use settings_screen::rows::Run;
    Some(match run {
        Run::AnswerDelegated(index, allow) => OrgAction::AnswerDelegated(index, allow),
        Run::RemoveFromLibrary(index) => OrgAction::RemoveFromLibrary(index),
        Run::ImportLogins(_) | Run::ClearCookies | Run::Server(_) | Run::Team(_) => {
            return None;
        }
    })
}

fn settings_click(target: SettingsTarget, session: &Session) -> Vec<Action> {
    let mut actions = Vec::new();
    if session.voice_field_edit.is_some() && !matches!(target, SettingsTarget::Field(_)) {
        actions.push(Action::VoiceField(VoiceFieldAction::Save));
    }
    if session.server_link.focus.is_some() && !matches!(target, SettingsTarget::Field(_) | SettingsTarget::Run(_)) {
        actions.push(Action::ServerLink(crate::state::ServerAction::Blur));
    }
    actions.extend(settings_target_click(target, session));
    actions
}

fn settings_target_click(target: SettingsTarget, session: &Session) -> Vec<Action> {
    match target {
        SettingsTarget::Field(settings_screen::rows::Field::Server(field)) => {
            vec![Action::ServerLink(crate::state::ServerAction::Focus(field))]
        }
        SettingsTarget::Field(field) => {
            field.voice_field().map(|field| Action::VoiceField(VoiceFieldAction::Focus(field))).into_iter().collect()
        }
        SettingsTarget::Profile(target) => {
            use crate::state::browser_profiles::ProfileAction as A;
            use crate::view::target::ProfilesTarget as P;
            vec![Action::BrowserProfile(match target {
                P::New => A::New,
                P::Edit(id) => A::Select(id),
                P::Delete(id) => A::Delete(id),
                P::Assign(id) => A::Assign(id),
                P::Field(field) => A::Focus(field),
                P::Toggle(toggle) => A::Toggle(toggle),
                P::WebRtc(webrtc) => A::SetWebRtc(webrtc),
                P::Save => A::Save,
                P::Cancel => A::Cancel,
            })]
        }
        SettingsTarget::Plugin(plugin) => plugins_click(plugin, session),
        SettingsTarget::Back => vec![Action::CloseSettings],
        SettingsTarget::Pane(pane) => vec![Action::ShowSettingsPane(pane)],
        SettingsTarget::Choose(setting) => vec![Action::ChooseSetting(setting)],
        SettingsTarget::Run(settings_screen::rows::Run::ImportLogins(kind)) => vec![Action::ImportLogins(kind)],
        SettingsTarget::Run(settings_screen::rows::Run::ClearCookies) => vec![Action::ClearBrowserCookies],
        SettingsTarget::Run(settings_screen::rows::Run::Server(action)) => vec![Action::ServerLink(action.into())],
        SettingsTarget::Run(settings_screen::rows::Run::Team(choice)) => {
            vec![Action::ChooseSetting(crate::state::Setting::Team(choice))]
        }
        SettingsTarget::Run(run) => organization_click(run).into_iter().map(Action::Organization).collect(),
        SettingsTarget::Surface => Vec::new(),
    }
}

pub fn settings_key(session: &Session, command: &KeyCommand) -> Option<Action> {
    if session.settings.pane == crate::state::SettingsPane::Server && session.server_link.focus.is_some() {
        use crate::state::ServerAction as S;
        return match command {
            KeyCommand::CloseWindow => Some(Action::CloseSettings),
            KeyCommand::Escape => Some(Action::ServerLink(S::Blur)),
            KeyCommand::Tab { back } => Some(Action::ServerLink(S::Step(!*back))),
            KeyCommand::Edit(key) => Some(Action::ServerLink(S::Edit(key.clone()))),
            KeyCommand::Submit => Some(Action::ServerLink(S::Submit)),
            _ => None,
        };
    }
    if session.voice_field_edit.is_some() {
        return match command {
            KeyCommand::CloseWindow => Some(Action::CloseSettings),
            KeyCommand::Escape => Some(Action::VoiceField(VoiceFieldAction::Cancel)),
            KeyCommand::Edit(key) => Some(Action::VoiceField(VoiceFieldAction::Edit(key.clone()))),
            KeyCommand::Submit => Some(Action::VoiceField(VoiceFieldAction::Save)),
            _ => None,
        };
    }
    if session.settings.pane == crate::state::SettingsPane::Browser && session.browser_profiles.editor.is_some() {
        use crate::state::browser_profiles::ProfileAction as A;
        return match command {
            KeyCommand::CloseWindow => Some(Action::CloseSettings),
            KeyCommand::Escape => Some(Action::BrowserProfile(A::Cancel)),
            KeyCommand::Tab { back } => Some(Action::BrowserProfile(A::NextField { back: *back })),
            KeyCommand::Edit(key) => Some(Action::BrowserProfile(A::Edit(key.clone()))),
            KeyCommand::Submit => Some(Action::BrowserProfile(A::Save)),
            _ => None,
        };
    }
    let sheet =
        session.plugins_pane_open() && (session.plugins.panel.sheet.is_some() || session.plugins.panel.local_models);
    match command {
        KeyCommand::CloseWindow => Some(Action::CloseSettings),
        KeyCommand::Escape => Some(Action::PluginsBack),
        KeyCommand::Tab { back } if sheet => Some(Action::StepConnectFocus { back: *back }),
        KeyCommand::Edit(key) if sheet => Some(Action::Edit(key.clone())),
        KeyCommand::Submit if sheet => Some(Action::Submit),
        _ => None,
    }
}

fn routines_click(target: RoutinesTarget) -> Option<RoutineAction> {
    use RoutineAction as A;
    Some(match target {
        RoutinesTarget::Surface => return None,
        RoutinesTarget::Back => A::Close,
        RoutinesTarget::New => A::New,
        RoutinesTarget::RunNow(id) => A::RunNow(id),
        RoutinesTarget::Toggle(id) => A::Toggle(id),
        RoutinesTarget::Revise(id) => A::Revise(id),
        RoutinesTarget::Delete(id) => A::Delete(id),
        RoutinesTarget::Field(field) => A::Focus(field),
        RoutinesTarget::Cadence(cadence) => A::Cadence(cadence),
        RoutinesTarget::Autonomy(autonomy) => A::Autonomy(autonomy),
        RoutinesTarget::Template(template) => A::Template(template),
        RoutinesTarget::Account(id) => A::Account(id),
        RoutinesTarget::Answers => A::ToggleAnswers,
        RoutinesTarget::Webhook => A::ToggleWebhook,
        RoutinesTarget::CopyWebhook(id) => A::CopyWebhook(id),
        RoutinesTarget::ApproveDraft(connection, draft) => A::ApproveDraft(connection, draft),
        RoutinesTarget::EditDraft(connection, draft) => A::EditDraft(connection, draft),
        RoutinesTarget::DraftField => return None,
        RoutinesTarget::DiscardDraft(connection, draft) => A::DiscardDraft(connection, draft),
        RoutinesTarget::Save => A::Save,
        RoutinesTarget::Cancel => A::Back,
        RoutinesTarget::Report(run) => A::Select(run),
        RoutinesTarget::Continue(run) => A::Continue(run),
    })
}

pub fn routines_key(session: &Session, command: &KeyCommand) -> Option<Action> {
    let form = session.routines.form.is_some() || session.channels.editing.is_some();
    let action = match command {
        KeyCommand::CloseWindow => RoutineAction::Close,
        KeyCommand::Escape => RoutineAction::Back,
        KeyCommand::Tab { back } if form => RoutineAction::NextField { back: *back },
        KeyCommand::Edit(key) if form => RoutineAction::Edit(key.clone()),
        KeyCommand::Submit if form => RoutineAction::Submit,
        _ => return None,
    };
    Some(Action::Routines(action))
}

fn plugins_click(target: PluginsTarget, session: &Session) -> Vec<Action> {
    let selected = || session.plugins.selected().map(|plugin| plugin.id.clone());
    let listed = |i: usize| session.plugins.list.get(i).map(|plugin| plugin.id.clone());
    match target {
        PluginsTarget::Fold if session.plugins.panel.sheet.is_some() => vec![Action::PluginsBack, Action::PluginsBack],
        PluginsTarget::Fold => vec![Action::PluginsBack],
        PluginsTarget::AddFolder => vec![Action::RequestPluginFolder],
        PluginsTarget::LocalModels => vec![Action::ToggleLocalModels],
        PluginsTarget::LocalModelField => vec![Action::FocusLocalModel],
        PluginsTarget::LocalModelPicker => vec![Action::ToggleLocalModelPicker],
        PluginsTarget::LocalModelChoice(i) => vec![Action::ChooseLocalModel(i)],
        PluginsTarget::DownloadModel => vec![Action::DownloadLocalModel],
        PluginsTarget::AiConnect(i) => listed(i).map(Action::ConnectPlugin).into_iter().collect(),
        PluginsTarget::Get(i) => listed(i).map(Action::InstallPlugin).into_iter().collect(),
        PluginsTarget::Open(i) => listed(i).map(Action::SelectPlugin).into_iter().collect(),
        PluginsTarget::Uninstall => selected().map(Action::UninstallPlugin).into_iter().collect(),
        PluginsTarget::AddAccount if session.settings.pane == crate::state::SettingsPane::Ai => {
            vec![Action::BeginAiAccount]
        }
        PluginsTarget::AddAccount => selected().map(Action::BeginConnect).into_iter().collect(),
        PluginsTarget::Disconnect(id) => vec![Action::Disconnect(id)],
        PluginsTarget::CopyHook(id) => vec![Action::CopyChannelHook(id)],
        PluginsTarget::Field(i) => vec![Action::FocusConnectField(i)],
        PluginsTarget::Connect => vec![Action::ConfirmConnect],
        PluginsTarget::CancelSheet => vec![Action::PluginsBack],
    }
}

fn review_click(target: ReviewTarget) -> Vec<Action> {
    use superfik_ui::components::AnnotationKnob as Knob;
    match target {
        ReviewTarget::Back => vec![Action::CloseReview],
        ReviewTarget::File(i) => vec![Action::SelectReviewFile(i)],
        ReviewTarget::Layout(layout) => vec![Action::SetDiffLayout(layout)],
        ReviewTarget::Line { side, line } => vec![Action::SelectDiffLine { side, line, extend: false }],
        ReviewTarget::AddComment => vec![Action::AddReviewComment],
        ReviewTarget::AskAgent => vec![Action::AskAboutSelection],
        ReviewTarget::Approve => vec![Action::BeginPublishReview(superfik_core::ReviewVerdict::Approve)],
        ReviewTarget::RequestChanges => vec![Action::BeginPublishReview(superfik_core::ReviewVerdict::RequestChanges)],
        ReviewTarget::Knob { comment, knob } => vec![match knob {
            Knob::Edit => Action::EditReviewComment(comment),
            Knob::Delete => Action::DeleteReviewComment(comment),
            Knob::Resolve => Action::ResolveReviewThread { id: comment, resolved: true },
            Knob::Reopen => Action::ResolveReviewThread { id: comment, resolved: false },
            Knob::Save => Action::SaveReviewComment,
            Knob::Cancel => Action::CancelReviewComment,
        }],
        ReviewTarget::Outgoing(id) => vec![Action::ToggleOutgoingComment(id)],
        ReviewTarget::Publish => vec![Action::PublishReview],
        ReviewTarget::CancelPublish => vec![Action::CancelPublishReview],
        ReviewTarget::Surface => Vec::new(),
    }
}

fn close_viewer(session: &Session) -> Option<Action> {
    session.conversation().expanded_reference.map(Action::ToggleReference)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{Skill, SkillIcon};

    use super::*;

    fn kinds(actions: &[Action]) -> Vec<String> {
        actions.iter().map(|a| format!("{a:?}").split(['(', ' ']).next().unwrap_or_default().to_owned()).collect()
    }

    #[test]
    fn the_server_panes_buttons_and_fields_reach_its_state() {
        use crate::state::{ServerAction, ServerField};
        use settings_screen::rows::{Field, Run, ServerRun};
        let session = Session::default();
        let join = Target::Settings(SettingsTarget::Run(Run::Server(ServerRun::Join)));
        assert!(matches!(click(Some(join), &session).as_slice(), [Action::ServerLink(ServerAction::Join)]));
        let field = Target::Settings(SettingsTarget::Field(Field::Server(ServerField::Code)));
        let focus = click(Some(field), &session);
        assert!(matches!(focus.as_slice(), [Action::ServerLink(ServerAction::Focus(ServerField::Code))]));
    }

    #[test]
    fn the_lock_screens_button_unlocks() {
        let unlock = click(Some(Target::Lock(LockTarget::Unlock)), &Session::default());
        assert!(matches!(unlock.as_slice(), [Action::Lock(LockAction::Unlock)]));
    }

    #[test]
    fn keys_answer_the_ask_on_screen_unless_something_else_has_them() {
        use superfik_core::{AgentAsk, AskId, AskKind, EngineEvent};
        let mut session = Session::new(Some(PathBuf::from("/repo")));
        let insert = |t: &str| KeyCommand::Edit(EditKey::Insert(t.into()));
        assert!(ask_key(&session, &KeyCommand::Submit).is_none(), "nothing asked");
        session.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        let run = session.conversation().run.as_ref().map(|r| r.id).expect("running");
        let kind = AskKind::Permission { tool: "Bash".into(), subject: "ls".into(), rememberable: true };
        let event = EngineEvent::Asked(AgentAsk { id: AskId("a".into()), kind });
        session.apply(Action::Server(superfik_protocol::ServerEvent::Run { run, event }), 0.0);

        let key = |session: &Session, command: KeyCommand| ask_key(session, &command).map(|a| format!("{a:?}"));
        assert_eq!(key(&session, KeyCommand::Submit).as_deref(), Some("Reply(Allow)"));
        assert_eq!(key(&session, KeyCommand::SubmitAlways).as_deref(), Some("Reply(AllowAlways)"));
        assert_eq!(key(&session, KeyCommand::Escape).as_deref(), Some("Reply(Deny)"));
        assert_eq!(key(&session, insert("2")).as_deref(), Some("PickChoice(1)"));
        assert_eq!(key(&session, KeyCommand::Edit(EditKey::End { select: false })).as_deref(), Some("MoveChoice(1)"));
        assert_eq!(key(&session, insert("0")), None);
        assert_eq!(key(&session, insert("x")), None);
        assert_eq!(key(&session, KeyCommand::NewConversation), None);
        session.popover = Some(PopoverKind::Permission);
        assert_eq!(key(&session, KeyCommand::Escape), None, "Esc closes the popover first");
    }

    #[test]
    fn agent_cards_open_their_session_and_escape_closes_it() {
        let mut session = Session::new(Some(PathBuf::from("/repo")));
        assert_eq!(format!("{:?}", click(Some(Target::Agent(1)), &session)), "[ToggleAgent(1)]");
        assert_eq!(format!("{:?}", click(Some(Target::CloseAgent), &session)), "[CloseAgent]");
        session.conversation_mut().open_agent = Some(0);
        assert_eq!(format!("{:?}", escape(&session)), "CloseAgent");
    }

    #[test]
    fn arrows_and_enter_drive_the_skill_picker() {
        let skill = |name: &str| Skill {
            name: name.into(),
            title: name.into(),
            description: String::new(),
            icon: SkillIcon::Cube,
        };
        let mut session = Session::new(Some(PathBuf::from("/repo")));
        session.available_skills = vec![skill("dither"), skill("motion")];
        let down = KeyCommand::Edit(EditKey::End { select: false });
        assert!(picker_key(&session, &down).is_none(), "picker closed");
        session.apply(Action::Edit(EditKey::Insert("/".into())), 0.0);
        let key = picker_key(&session, &down).expect("picker has the arrows");
        assert_eq!(format!("{key:?}"), "MovePick(1)");
        session.apply(key, 0.0);
        assert_eq!(format!("{:?}", enter(&session)), "CompleteSkill(\"motion\")");
        let right = KeyCommand::Edit(EditKey::Right { select: false });
        assert!(picker_key(&session, &right).is_none(), "one page: → moves the caret");
        let shift_down = KeyCommand::Edit(EditKey::End { select: true });
        assert!(picker_key(&session, &shift_down).is_none(), "selecting text stays with the composer");
    }

    #[test]
    fn the_permission_chip_opens_its_menu_and_items_set_the_mode() {
        let session = Session::default();
        let mode = superfik_core::PermissionMode::Plan;
        assert_eq!(kinds(&click(Some(Target::Composer(ComposerPart::Permission)), &session)), vec!["OpenPopover"]);
        assert_eq!(kinds(&click(Some(Target::PermissionItem(mode)), &session)), vec!["SetPermissionMode"]);
        let reply = Target::Ask(AskTarget::Reply(Reply::Deny));
        assert_eq!(kinds(&click(Some(reply), &session)), vec!["Reply"]);
        assert_eq!(kinds(&click(Some(Target::Ask(AskTarget::Option(1))), &session)), vec!["PickChoice"]);
    }

    #[test]
    fn pager_arrows_flip_their_node() {
        let session = Session::default();
        let plugins = crate::state::Carousel::Plugins;
        assert!(matches!(click(Some(Target::Page(plugins, 1)), &session)[..], [Action::Page(_, 1)]));
        assert!(matches!(click(Some(Target::Page(plugins, -1)), &session)[..], [Action::Page(_, -1)]));
    }

    #[test]
    fn viewer_closes_from_its_scrim_knob_or_escape() {
        let mut session = Session::default();
        session.apply(Action::AddReferences(vec![PathBuf::from("/a.png"), PathBuf::from("/b.png")]), 0.0);
        assert!(click(Some(Target::Scrim), &session).is_empty(), "nothing to close");
        assert!(matches!(escape(&session), Action::ClosePopover));
        session.apply(Action::ToggleReference(1), 0.0);
        assert!(matches!(click(Some(Target::Scrim), &session)[..], [Action::ToggleReference(1)]));
        assert!(matches!(click(Some(Target::CloseViewer), &session)[..], [Action::ToggleReference(1)]));
        assert!(matches!(escape(&session), Action::ToggleReference(1)));
    }

    #[test]
    fn repository_opens_recents_or_the_picker() {
        let mut session = Session::default();
        assert_eq!(kinds(&click(Some(Target::AddRepository), &session)), vec!["RequestWorkspace"]);
        session.recent_workspaces = vec![PathBuf::from("/a")];
        assert_eq!(kinds(&click(Some(Target::AddRepository), &session)), vec!["OpenPopover"]);
        assert_eq!(kinds(&click(Some(Target::RemoveWorkspace), &session)), vec!["RemoveWorkspace"]);
        session.popover = Some(PopoverKind::Repository);
        assert_eq!(kinds(&click(Some(Target::RepositoryItem(0)), &session)), vec!["OpenWorkspace"]);
    }

    #[test]
    fn clicking_outside_closes_popover() {
        let session = Session { popover: Some(PopoverKind::Model), ..Session::default() };
        assert_eq!(kinds(&click(None, &session)), vec!["ClosePopover"]);
    }

    #[test]
    fn send_submits_or_cancels() {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        assert_eq!(kinds(&click(Some(Target::Composer(ComposerPart::Send)), &session)), vec!["Submit"]);
        session.apply(Action::Edit(crate::state::EditKey::Insert("go".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        assert_eq!(kinds(&click(Some(Target::Composer(ComposerPart::Send)), &session)), vec!["Cancel"]);
    }

    #[test]
    fn escape_interrupts_the_run_once_the_popover_is_closed() {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        assert!(matches!(escape(&session), Action::ClosePopover), "idle: nothing to interrupt");
        session.apply(Action::Edit(crate::state::EditKey::Insert("go".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        session.popover = Some(PopoverKind::Model);
        assert!(matches!(escape(&session), Action::ClosePopover), "the popover closes first");
        session.popover = None;
        assert!(matches!(escape(&session), Action::Cancel));
    }

    #[test]
    fn the_expand_button_toggles_and_escape_collapses_after_leaving_the_address() {
        let mut session = Session::default();
        session.apply(Action::OpenBrowser, 0.0);
        let expand = click(Some(Target::Browser(BrowserPart::Expand)), &session);
        assert_eq!(kinds(&expand), vec!["FocusBrowser", "ToggleBrowserExpanded"]);
        session.apply(Action::ToggleBrowserExpanded, 0.0);
        assert!(matches!(escape(&session), Action::FocusBrowser(None)), "the address first");
        session.apply(Action::FocusBrowser(None), 0.0);
        assert!(matches!(escape(&session), Action::ToggleBrowserExpanded));
    }

    #[test]
    fn the_header_expands_and_escape_leaves_the_page() {
        let mut session = Session::default();
        session.apply(Action::OpenBrowser, 0.0);
        let header = click(Some(Target::Browser(BrowserPart::Header)), &session);
        assert_eq!(kinds(&header), vec!["FocusBrowser", "ToggleBrowserExpanded"]);
        session.apply(Action::FocusBrowser(Some(BrowserFocus::Page)), 0.0);
        assert!(app_owns_key(&KeyCommand::Escape), "the page never swallows Esc");
        assert!(matches!(escape(&session), Action::FocusBrowser(None)));
    }

    #[test]
    fn the_key_opens_the_logins_and_a_login_fills_or_expanded_fills_the_first() {
        let mut session = Session::default();
        session.apply(Action::OpenBrowser, 0.0);
        let key = click(Some(Target::Browser(BrowserPart::Logins)), &session);
        assert!(matches!(key.last(), Some(Action::OpenPopover(PopoverKind::Logins))), "{key:?}");
        session.popover = Some(PopoverKind::Logins);
        let picked = click(Some(Target::LoginItem(1)), &session);
        assert!(matches!(picked.first(), Some(Action::ClosePopover)), "{picked:?}");
        assert!(matches!(picked.last(), Some(Action::FillLogin(1))), "{picked:?}");
        session.popover = None;
        session.apply(Action::ToggleBrowserExpanded, 0.0);
        let key = click(Some(Target::Browser(BrowserPart::Logins)), &session);
        assert!(matches!(key.last(), Some(Action::FillLogin(0))), "{key:?}");
        assert!(app_owns_key(&KeyCommand::FillLogin), "⌘\\ works over the page");
    }

    #[test]
    fn status_line_toggles_steps_and_notice_closes() {
        let session = Session::default();
        assert_eq!(kinds(&click(Some(Target::Steps(0)), &session)), vec!["ToggleSteps"]);
        assert_eq!(kinds(&click(Some(Target::DismissNotice), &session)), vec!["DismissNotice"]);
    }

    #[test]
    fn timeline_rows_switch_start_or_archive_sessions() {
        let session = Session::default();
        let id = session.active;
        assert!(
            matches!(&click(Some(Target::Conversation(id)), &session)[..], [Action::SelectConversation(i)] if *i == id)
        );
        assert_eq!(kinds(&click(Some(Target::NewConversation), &session)), vec!["NewConversation"]);
        assert_eq!(kinds(&click(Some(Target::ArchiveConversation(id)), &session)), vec!["ArchiveConversation"]);
        assert_eq!(kinds(&click(Some(Target::DayGroup(3)), &session)), vec!["ToggleGroup"]);
    }

    #[test]
    fn archived_rows_restore_and_the_glyph_restores_without_opening() {
        let session = Session::default();
        let id = session.active;
        assert_eq!(kinds(&click(Some(Target::ArchivedGroup), &session)), vec!["ToggleArchived"]);
        assert_eq!(
            kinds(&click(Some(Target::ArchivedConversation(id)), &session)),
            vec!["UnarchiveConversation", "SelectConversation"]
        );
        assert_eq!(kinds(&click(Some(Target::UnarchiveConversation(id)), &session)), vec!["UnarchiveConversation"]);
    }

    #[test]
    fn clicking_away_from_a_title_being_edited_commits_it() {
        let mut session = Session::default();
        let id = session.active;
        session.apply(Action::BeginRename(id), 0.0);
        assert!(click(Some(Target::Conversation(id)), &session).is_empty(), "clicking the editor keeps editing");
        assert_eq!(kinds(&click(None, &session)), vec!["CommitRename"]);
        assert_eq!(kinds(&click(Some(Target::NewConversation), &session)), vec!["CommitRename", "NewConversation"]);
        assert!(matches!(enter(&session), Action::CommitRename));
        assert!(matches!(escape(&session), Action::CancelRename));
        session.apply(Action::CancelRename, 0.0);
        assert!(matches!(escape(&session), Action::ClosePopover));
    }

    #[test]
    fn settings_gear_opens_the_screen_and_closes_a_popover() {
        let mut session = Session::default();
        assert_eq!(kinds(&click(Some(Target::OpenSettings), &session)), vec!["OpenSettings"]);
        session.popover = Some(PopoverKind::Model);
        assert_eq!(kinds(&click(Some(Target::OpenSettings), &session)), vec!["ClosePopover", "OpenSettings"]);
    }

    #[test]
    fn the_settings_screen_picks_sections_and_settings_and_goes_back() {
        use crate::state::{Setting, SettingsPane};
        let mut session = Session::default();
        session.apply(Action::OpenSettings, 0.0);
        let at = |target| kinds(&click(Some(Target::Settings(target)), &session));
        assert_eq!(at(SettingsTarget::Pane(SettingsPane::Sound)), vec!["ShowSettingsPane"]);
        assert_eq!(at(SettingsTarget::Choose(Setting::Haptics(false))), vec!["ChooseSetting"]);
        assert_eq!(at(SettingsTarget::Back), vec!["CloseSettings"]);
        assert!(at(SettingsTarget::Surface).is_empty(), "the screen itself is inert");
        assert!(matches!(escape(&session), Action::PluginsBack));
    }

    #[test]
    fn the_voice_fields_take_clicks_and_keys_and_a_click_away_keeps_them() {
        use crate::state::VoiceField;
        use crate::view::settings_screen::rows::Field;
        let mut session = Session::default();
        session.apply(Action::OpenSettings, 0.0);
        let field = Target::Settings(SettingsTarget::Field(Field::WakeWord));
        assert!(matches!(
            click(Some(field), &session).as_slice(),
            [Action::VoiceField(VoiceFieldAction::Focus(VoiceField::WakeWord))]
        ));
        let address = Target::Settings(SettingsTarget::Field(Field::VoiceAddress));
        assert!(matches!(
            click(Some(address), &session).as_slice(),
            [Action::VoiceField(VoiceFieldAction::Focus(VoiceField::Address))]
        ));
        session.apply(Action::VoiceField(VoiceFieldAction::Focus(VoiceField::WakeWord)), 0.0);
        let key = |command| settings_key(&session, &command);
        assert!(matches!(
            key(KeyCommand::Edit(EditKey::Insert("a".into()))),
            Some(Action::VoiceField(VoiceFieldAction::Edit(_)))
        ));
        assert!(matches!(key(KeyCommand::Submit), Some(Action::VoiceField(VoiceFieldAction::Save))));
        assert!(matches!(key(KeyCommand::Escape), Some(Action::VoiceField(VoiceFieldAction::Cancel))));
        let away = click(Some(Target::Settings(SettingsTarget::Back)), &session);
        assert!(
            matches!(away.as_slice(), [Action::VoiceField(VoiceFieldAction::Save), Action::CloseSettings]),
            "{away:?}"
        );
    }

    #[test]
    fn over_settings_only_escape_and_command_w_act() {
        let session = Session::default();
        assert!(matches!(settings_key(&session, &KeyCommand::Escape), Some(Action::PluginsBack)));
        assert!(matches!(settings_key(&session, &KeyCommand::CloseWindow), Some(Action::CloseSettings)));
        assert!(settings_key(&session, &KeyCommand::Edit(EditKey::Insert("a".into()))).is_none());
        assert!(settings_key(&session, &KeyCommand::Submit).is_none());
        assert!(settings_key(&session, &KeyCommand::Tab { back: false }).is_none());
    }

    #[test]
    fn an_open_account_sheet_takes_typing_and_enter_in_settings() {
        let mut session = Session::default();
        session.apply(Action::OpenPlugins, 0.0);
        session.plugins.panel.sheet = Some(crate::state::ConnectSheet {
            plugin: superfik_core::PluginId("mail".into()),
            label: Default::default(),
            fields: Vec::new(),
            focus: 0,
        });
        let typed = KeyCommand::Edit(EditKey::Insert("a".into()));
        assert!(matches!(settings_key(&session, &typed), Some(Action::Edit(_))));
        assert!(matches!(settings_key(&session, &KeyCommand::Submit), Some(Action::Submit)));
        let tab = KeyCommand::Tab { back: true };
        assert!(matches!(settings_key(&session, &tab), Some(Action::StepConnectFocus { back: true })));
    }

    #[test]
    fn model_item_selects_without_closing_first() {
        let session = Session { popover: Some(PopoverKind::Model), ..Session::default() };
        assert_eq!(kinds(&click(Some(Target::ModelItem(2)), &session)), vec!["SelectModel"]);
    }

    #[test]
    fn branch_items_select_branch_refs() {
        let mut session = Session::default();
        session.snapshot.repository = Some(superfik_core::Repository {
            branches: vec!["main".into()],
            remote_branches: vec!["spike".into()],
            ..superfik_core::Repository::default()
        });
        session.popover = Some(PopoverKind::Branch);
        let actions = click(Some(Target::BranchItem(1)), &session);
        assert!(matches!(&actions[..], [Action::SelectBranch(b)] if *b == superfik_core::BranchRef::remote("spike")));
    }

    #[test]
    fn github_rows_open_or_attach_through_the_backend() {
        let mut session = Session::new(Some("/r".into()));
        session.github = Some(crate::state::GitHubListing { signed_in: true, repositories: vec![] });
        assert_eq!(kinds(&click(Some(Target::AddRepository), &session)), vec!["OpenPopover"]);
        session.popover = Some(PopoverKind::Repository);
        assert_eq!(kinds(&click(Some(Target::GitHubRepositoryItem(0)), &session)), vec!["OpenGitHubRepository"]);
        session.popover = Some(PopoverKind::AttachRepository);
        assert_eq!(
            kinds(&click(Some(Target::AttachGitHubRepositoryItem(1)), &session)),
            vec!["AttachGitHubRepository"]
        );
    }

    #[test]
    fn the_review_screen_routes_its_controls_and_escape_steps_out() {
        let mut session = Session::new(Some("/r".into()));
        session.conversation_mut().pull_request = Some(superfik_core::PullRequest { number: 7, ..Default::default() });
        assert_eq!(kinds(&click(Some(Target::OpenReview), &session)), vec!["OpenReview"]);
        assert_eq!(kinds(&click(Some(Target::Close(NodeKey::PullRequest)), &session)), vec!["RemovePullRequest"]);
        session.apply(Action::OpenReview, 0.0);
        assert!(matches!(escape(&session), Action::CloseReview));
        assert_eq!(kinds(&click(Some(Target::Review(ReviewTarget::File(1))), &session)), vec!["SelectReviewFile"]);
        let knob = ReviewTarget::Knob { comment: 3, knob: superfik_ui::components::AnnotationKnob::Delete };
        assert!(matches!(click(Some(Target::Review(knob)), &session)[..], [Action::DeleteReviewComment(3)]));
        assert_eq!(kinds(&click(Some(Target::Review(ReviewTarget::Approve)), &session)), vec!["BeginPublishReview"]);
        session.apply(Action::BeginPublishReview(superfik_core::ReviewVerdict::Approve), 0.0);
        assert!(matches!(escape(&session), Action::CancelPublishReview));
    }

    #[test]
    fn the_pull_request_sheet_routes_its_controls_and_escape_cancels() {
        let mut session = Session::new(Some("/r".into()));
        assert_eq!(kinds(&click(Some(Target::OpenPullRequest), &session)), vec!["ProposePullRequest"]);
        session.pr_composer = Some(crate::state::PullRequestComposer::default());
        assert_eq!(kinds(&click(Some(Target::ConfirmPullRequest), &session)), vec!["ConfirmPullRequest"]);
        assert_eq!(kinds(&click(Some(Target::WritePullRequestWithAgent), &session)), vec!["RequestPullRequestDraft"]);
        assert!(matches!(
            click(Some(Target::PullRequestField(crate::state::PrField::Body)), &session)[..],
            [Action::FocusPullRequestField(crate::state::PrField::Body)]
        ));
        assert!(matches!(escape(&session), Action::CancelPullRequest));
    }

    #[test]
    fn attaching_a_repository_lists_recents_or_opens_the_dialog() {
        let mut session = Session::new(Some("/r".into()));
        assert_eq!(kinds(&click(Some(Target::AddRepository), &session)), vec!["RequestRepository"]);
        session.recent_workspaces = vec!["/r".into(), "/engine".into()];
        assert_eq!(kinds(&click(Some(Target::AddRepository), &session)), vec!["OpenPopover"]);
        session.popover = Some(PopoverKind::AttachRepository);
        let actions = click(Some(Target::AttachRepositoryItem(0)), &session);
        assert!(matches!(&actions[..], [Action::AddRepositoryPath(p)] if p == std::path::Path::new("/engine")));
        assert_eq!(
            kinds(&click(Some(Target::ChooseAttachedRepository), &session)),
            vec!["ClosePopover", "RequestRepository"]
        );
        assert_eq!(
            kinds(&click(Some(Target::RemoveRepository(0)), &session)),
            vec!["ClosePopover", "RemoveRepository"]
        );
    }

    #[test]
    fn left_and_right_turn_the_pickers_pages_when_there_are_several() {
        let skill = |i: usize| Skill {
            name: format!("skill-{i:02}"),
            title: String::new(),
            description: String::new(),
            icon: SkillIcon::Cube,
        };
        let mut session = Session::new(Some(PathBuf::from("/repo")));
        session.available_skills = (0..12).map(skill).collect();
        session.apply(Action::Edit(EditKey::Insert("/skill".into())), 0.0);
        let right = KeyCommand::Edit(EditKey::Right { select: false });
        let left = KeyCommand::Edit(EditKey::Left { select: false });
        let next = picker_key(&session, &right).expect("→ turns the page");
        assert_eq!(format!("{next:?}"), "PagePick(Skills, 1)");
        let back = picker_key(&session, &left).expect("← turns it back");
        assert_eq!(format!("{back:?}"), "PagePick(Skills, -1)");
    }

    #[test]
    fn tab_accepts_the_completion_and_otherwise_keeps_its_focus_meaning() {
        let mut session = Session::default();
        session.available_skills.push(Skill {
            name: "test".into(),
            title: "Test skill".into(),
            description: String::new(),
            icon: SkillIcon::Bolt,
        });
        let tab = KeyCommand::Tab { back: false };
        assert!(picker_key(&session, &tab).is_none(), "nothing typed, nothing to complete");
        session.apply(Action::Edit(crate::state::EditKey::Insert("/te".into())), 0.0);
        assert!(matches!(picker_key(&session, &tab), Some(Action::AcceptCompletion)));
        assert!(picker_key(&session, &KeyCommand::Tab { back: true }).is_none(), "⇧Tab still moves focus");
    }

    #[test]
    fn enter_completes_skill_while_picking() {
        let mut session = Session::default();
        session.available_skills.push(Skill {
            name: "motion".into(),
            title: "Motion skill".into(),
            description: String::new(),
            icon: SkillIcon::Bolt,
        });
        session.apply(Action::Edit(crate::state::EditKey::Insert("/mo".into())), 0.0);
        assert!(matches!(enter(&session), Action::CompleteSkill(name) if name == "motion"));
        session.apply(Action::ClosePopover, 0.0);
        assert!(matches!(enter(&session), Action::Submit));
    }

    #[test]
    fn memory_clicks_open_select_and_give_keys_back() {
        use superfik_core::MemoryId;
        let mut session = Session::new(Some(PathBuf::from("/r")));
        let memory = |t| Some(Target::Memory(t));
        assert_eq!(kinds(&click(Some(Target::Node(NodeKey::Memory)), &session)), vec!["OpenMemory"]);
        assert_eq!(
            kinds(&click(memory(MemoryTarget::Item(MemoryId(3))), &session)),
            vec!["OpenMemory", "SelectMemory"]
        );
        session.apply(Action::FocusMemory(Some(MemoryField::Search)), 0.0);
        assert!(click(memory(MemoryTarget::Search), &session).iter().all(|a| !matches!(a, Action::FocusMemory(None))));
        assert_eq!(kinds(&click(Some(Target::Composer(ComposerPart::Image)), &session))[0], "FocusMemory");
        assert!(matches!(escape(&session), Action::CloseMemory));
        session.apply(Action::OpenMemory, 0.0);
        assert_eq!(kinds(&click(memory(MemoryTarget::Item(MemoryId(3))), &session)), vec!["SelectMemory"]);
        assert_eq!(kinds(&click(memory(MemoryTarget::Close), &session)), vec!["FocusMemory", "CloseMemory"]);
        assert!(click(memory(MemoryTarget::Surface), &session).is_empty(), "the open explorer keeps its focus");
        assert!(click(memory(MemoryTarget::Graph), &session).is_empty(), "nothing selected, nothing to drop");
        session.apply(Action::SelectMemory(MemoryId(3)), 0.0);
        assert_eq!(kinds(&click(memory(MemoryTarget::Graph), &session)), vec!["SelectMemory"]);
    }
}
