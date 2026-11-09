//! Which sound an action makes. A product rule, so it lives in the reducer
//! and is pure: it looks at what the action was and what changed.

use superfik_core::EngineEvent;
use superfik_protocol::ServerEvent;
use superfik_sound::Cue;

use super::{Action, EditKey, OrgAction, PopoverKind, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Trigger {
    Insert,
    Submit,
    Cancel,
    ToggleDraft,
    ToggleAgent,
    SetEffort,
    SelectModel,
    OpenPopover,
    ClosePopover,
    OpenBrowser,
    Add,
    Remove,
    CompleteSkill,
    ToggleReference,
    OpenWorkspace,
    ToggleSteps,
    ToggleGroup,
    Restore,
    DismissNotice,
    NewConversation,
    SelectConversation,
    StepConversation,
    ScrollConversation,
    ChooseSetting,
    ToggleSettings,
    SettingsPane,
    CompactContext,
    RunCommand,
    RunCompleted,
    RunFailed,
    RunCancelled,
    Diagnosed,
    ServerError,
    WorkspaceRefreshed,
    ToggleMemory,
    Remember,
    Forget,
    Asked,
    Answer,
    Silent,
    Woke,
    HoldTalk,
    Routines,
    Press,
    Discard,
}

impl Trigger {
    pub(super) fn of(action: &Action) -> Self {
        match action {
            Action::BrowserProfile(_) => Trigger::Silent,
            Action::VoiceField(super::VoiceFieldAction::Edit(EditKey::Insert(_))) => Trigger::Insert,
            Action::VoiceField(_) => Trigger::Silent,
            Action::Voice(super::VoiceAction::Toggle) => Trigger::Press,
            Action::Voice(super::VoiceAction::HoldTalk) => Trigger::HoldTalk,
            Action::Voice(super::VoiceAction::ReleaseTalk | super::VoiceAction::DropTalk) => Trigger::Silent,
            Action::Voice(super::VoiceAction::Device(superfik_voice::VoiceEvent::Woke { rest }))
                if rest.trim().is_empty() =>
            {
                Trigger::Woke
            }
            Action::Voice(super::VoiceAction::Device(_)) => Trigger::Silent,
            Action::Routines(action) => {
                use super::RoutineAction as R;
                match action {
                    R::Edit(EditKey::Insert(_)) => Trigger::Insert,
                    R::Edit(_) | R::Focus(_) | R::NextField { .. } | R::Scroll { .. } | R::EditDraft(..) => {
                        Trigger::Silent
                    }
                    R::Toggle(_) | R::RunNow(_) | R::ApproveDraft(..) => Trigger::Press,
                    R::Delete(_) | R::DiscardDraft(..) => Trigger::Discard,
                    R::Cadence(_) | R::Autonomy(_) | R::Template(_) | R::Account(_) | R::ToggleAnswers => {
                        Trigger::SetEffort
                    }
                    _ => Trigger::Routines,
                }
            }
            Action::Edit(EditKey::Insert(_)) | Action::AcceptCompletion => Trigger::Insert,
            Action::Edit(_) => Trigger::Silent,
            Action::Submit | Action::RetryTurn(_) | Action::VisualPrompt(_) => Trigger::Submit,
            Action::Cancel => Trigger::Cancel,
            Action::CompactContext => Trigger::CompactContext,
            Action::RunCommand(_) => Trigger::RunCommand,
            Action::PagePick(..) => Trigger::Silent,
            Action::SetEffort(_) => Trigger::SetEffort,
            Action::SelectModel(_) => Trigger::SelectModel,
            Action::OpenPopover(_) => Trigger::OpenPopover,
            Action::ToggleAgent(_) | Action::CloseAgent => Trigger::ToggleAgent,
            Action::ClosePopover => Trigger::ClosePopover,
            Action::AddReferences(_) | Action::Server(ServerEvent::Repository(_)) => Trigger::Add,
            Action::ConfirmPullRequest | Action::PublishReview => Trigger::Submit,
            Action::CloseReview | Action::CancelPublishReview | Action::CancelReviewComment => Trigger::ClosePopover,
            Action::OpenReview | Action::BeginPublishReview(_) | Action::AddReviewComment => Trigger::OpenPopover,
            Action::SaveReviewComment => Trigger::Add,
            Action::DeleteReviewComment(_) => Trigger::Remove,
            Action::SelectReviewFile(_) | Action::SetDiffLayout(_) | Action::ToggleOutgoingComment(_) => {
                Trigger::SelectModel
            }
            Action::CancelPullRequest => Trigger::ClosePopover,
            Action::TogglePullRequestDraft => Trigger::ToggleDraft,
            Action::ProposePullRequest => Trigger::OpenPopover,
            Action::RemoveRepository(_)
            | Action::RemoveWorkspace
            | Action::RemovePullRequest
            | Action::RemoveSkill(_)
            | Action::RemoveReferences
            | Action::RemoveReference(_)
            | Action::RemoveAgent(_)
            | Action::RemoveConnection(_)
            | Action::Disconnect(_)
            | Action::UninstallPlugin(_)
            | Action::ClearResearch => Trigger::Remove,
            Action::PickConnection(_) | Action::UseConnection(_) | Action::AddSkill(_) => Trigger::CompleteSkill,
            Action::OpenPlugins | Action::BeginConnect(_) | Action::ConnectPlugin(_) | Action::OpenAccountMenu(_) => {
                Trigger::OpenPopover
            }
            Action::PluginsBack => Trigger::ClosePopover,
            Action::SelectPlugin(_) => Trigger::SelectModel,
            Action::InstallPlugin(_) | Action::InstallPluginFolder(_) => Trigger::Add,
            Action::ConfirmConnect => Trigger::Submit,
            Action::CopyChannelHook(_) => Trigger::Add,
            Action::Organization(OrgAction::AnswerDelegated(..)) => Trigger::Submit,
            Action::Lock(super::LockAction::Unlock) => Trigger::Submit,
            Action::Lock(super::LockAction::Failed(_)) => Trigger::RunFailed,
            Action::Lock(_) => Trigger::Silent,
            Action::ServerLink(super::ServerAction::Join | super::ServerAction::AddUser) => Trigger::Submit,
            Action::ServerLink(super::ServerAction::Joined(Err(_))) => Trigger::RunFailed,
            Action::ServerLink(super::ServerAction::CopyCode) => Trigger::Add,
            Action::ServerLink(super::ServerAction::Edit(super::EditKey::Insert(_))) => Trigger::Insert,
            Action::ServerLink(_) => Trigger::Silent,
            Action::Update(super::UpdateAction::Primary) => Trigger::Submit,
            Action::Update(super::UpdateAction::Ready) => Trigger::RunCompleted,
            Action::Update(super::UpdateAction::Failed(_)) => Trigger::RunFailed,
            Action::Update(_) => Trigger::Silent,
            Action::CompleteSkill(_) => Trigger::CompleteSkill,
            Action::ToggleReference(_) => Trigger::ToggleReference,
            Action::OpenWorkspace(_) => Trigger::OpenWorkspace,
            Action::ToggleSteps(_) | Action::ToggleStepOutput(..) => Trigger::ToggleSteps,
            Action::DismissNotice => Trigger::DismissNotice,
            Action::NewConversation => Trigger::NewConversation,
            Action::ChooseSetting(_) => Trigger::ChooseSetting,
            Action::OpenSettings | Action::CloseSettings => Trigger::ToggleSettings,
            Action::ShowSettingsPane(_) => Trigger::SettingsPane,
            Action::SelectConversation(_) | Action::SelectShortcut(_) => Trigger::SelectConversation,
            Action::StepConversation(_) => Trigger::StepConversation,
            Action::ScrollConversation(_) => Trigger::ScrollConversation,
            Action::ArchiveConversation(_) => Trigger::Remove,
            Action::BeginRename(_) | Action::CommitRename | Action::CancelRename => Trigger::Silent,
            Action::ToggleGroup(_) | Action::ToggleArchived => Trigger::ToggleGroup,
            Action::UnarchiveConversation(_) => Trigger::Restore,
            Action::OpenMemory | Action::CloseMemory => Trigger::ToggleMemory,
            Action::SelectMemory(_) | Action::SetMemoryKind(_) | Action::SetRevisionKind(_) => Trigger::SetEffort,
            Action::Remember => Trigger::Remember,
            Action::OptimizeMemory => Trigger::Submit,
            Action::UndoMemory => Trigger::Remove,
            Action::ForgetMemory(_) => Trigger::Forget,
            Action::ShareMemory(_) => Trigger::Add,
            Action::FocusMemory(_) | Action::ScrollMemory { .. } | Action::ReviseMemory(_) => Trigger::Silent,
            Action::Note(action) => action.trigger(),
            Action::ScrollAgent { .. } => Trigger::Silent,
            Action::OpenBrowser => Trigger::OpenBrowser,
            Action::CloseBrowser => Trigger::Remove,
            Action::FocusBrowser(_)
            | Action::BrowserBack
            | Action::BrowserForward
            | Action::Reload
            | Action::BrowserPage(..)
            | Action::BrowserSources(_)
            | Action::LoginsImported(..) => Trigger::Silent,
            Action::TogglePrivateBrowsing => Trigger::SetEffort,
            Action::ToggleBrowserExpanded => Trigger::ToggleReference,
            Action::ImportLogins(_) | Action::FillLogin(_) | Action::SaveLogin(_) | Action::UsePasskey(_) => {
                Trigger::Add
            }
            Action::DismissSaveLogin | Action::PasskeyElsewhere => Trigger::Silent,
            Action::ClearBrowserCookies | Action::Organization(OrgAction::RemoveFromLibrary(_)) => Trigger::Remove,
            Action::Reply(_) | Action::PickChoice(_) => Trigger::Answer,
            Action::SetPermissionMode(_) => Trigger::SetEffort,
            Action::MoveChoice(_) | Action::MovePick(_) => Trigger::Silent,
            Action::Server(ServerEvent::Run { event, .. }) => match event {
                EngineEvent::Completed { .. } => Trigger::RunCompleted,
                EngineEvent::Failed(_) => Trigger::RunFailed,
                EngineEvent::Cancelled => Trigger::RunCancelled,
                EngineEvent::Diagnosed(_) => Trigger::Diagnosed,
                EngineEvent::Asked(_) => Trigger::Asked,
                _ => Trigger::Silent,
            },
            Action::Server(ServerEvent::Error { .. }) => Trigger::ServerError,
            Action::Server(ServerEvent::Workspace(_)) => Trigger::WorkspaceRefreshed,
            Action::Server(_)
            | Action::SelectBranch(_)
            | Action::RestoreRecents(_)
            | Action::RestorePreferences(_)
            | Action::Managed(_)
            | Action::SystemAppearance(_)
            | Action::RequestWorkspace
            | Action::RequestReferences
            | Action::RequestRepository
            | Action::SelectPullRequest(_)
            | Action::RequestPullRequestDraft
            | Action::SelectDiffLine { .. }
            | Action::ScrollReview { .. }
            | Action::EditReviewComment(_)
            | Action::ResolveReviewThread { .. }
            | Action::AskAboutSelection
            | Action::DelegateReview
            | Action::FocusPullRequestField(_)
            | Action::OpenGitHubRepository(_)
            | Action::SignInToGitHub
            | Action::AttachGitHubRepository(_)
            | Action::AddRepositoryPath(_)
            | Action::RequestPluginFolder
            | Action::BeginAiAccount
            | Action::ToggleLocalModels
            | Action::FocusLocalModel
            | Action::ToggleLocalModelPicker
            | Action::ChooseLocalModel(_)
            | Action::DownloadLocalModel
            | Action::FocusConnectField(_)
            | Action::StepConnectFocus { .. }
            | Action::ScrollSettings { .. }
            | Action::ScrollModels { .. }
            | Action::Page(..)
            | Action::ShowGetStarted
            | Action::DismissGetStarted => Trigger::Silent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Probe {
    popover: Option<PopoverKind>,
    research: usize,
    draft: bool,
    open_agent: Option<usize>,
    running: bool,
    runs: usize,
    compacting: bool,
    active: superfik_core::ConversationId,
    notice: bool,
    turns: usize,
    attachments: usize,
    skills: usize,
    agents: usize,
    model: usize,
    expanded_reference: Option<usize>,
    open_steps: usize,
    branch: Option<String>,
    root: Option<std::path::PathBuf>,
    preferences: super::Preferences,
    settings: super::SettingsScreen,
    routines_open: bool,
    routine_form: bool,
    routine_report: Option<superfik_core::RunId>,
    memory_open: bool,
    memories: usize,
    memory_draft: bool,
    asks: usize,
    picks: usize,
    get_started: bool,
    listening: bool,
    talking: bool,
    archived: usize,
}

impl Probe {
    pub(super) fn of(session: &Session) -> Self {
        let conversation = session.conversation();
        Self {
            popover: session.popover,
            research: conversation.research_items.len(),
            draft: session.pr_composer.as_ref().is_some_and(|c| c.draft),
            open_agent: conversation.open_agent,
            running: conversation.is_running(),
            runs: session.conversations.iter().filter(|c| c.is_running()).count(),
            compacting: conversation.is_compacting(),
            active: session.active,
            notice: session.notice.is_some(),
            turns: conversation.turns.len(),
            attachments: conversation.repositories.len() + conversation.references.len(),
            skills: session.selected_skills().len(),
            agents: conversation.agents.len(),
            model: conversation.model,
            expanded_reference: conversation.expanded_reference,
            open_steps: conversation.open_steps.len(),
            branch: session.branch.clone(),
            root: session.root.clone(),
            preferences: session.preferences,
            settings: session.settings,
            routines_open: session.routines.open,
            routine_form: session.routines.form.is_some(),
            routine_report: session.routines.selected,
            memory_open: session.memory.open,
            memories: session.memory.memories.len(),
            memory_draft: !session.memory.draft.text().trim().is_empty(),
            asks: session.conversations.iter().map(|c| c.asks.len()).sum(),
            picks: session.pending_ask().map_or(0, |a| a.picks.iter().map(Vec::len).sum::<usize>() + a.question),
            get_started: session.get_started,
            listening: session.voice.phase == super::VoicePhase::Listening,
            talking: session.voice.is_on(),
            archived: session.conversations.iter().filter(|c| c.archived).count(),
        }
    }
}

impl Probe {
    fn removable(&self) -> usize {
        self.attachments + self.skills + self.agents + self.research
    }
}

pub(super) fn cue(trigger: Trigger, before: &Probe, after: &Probe) -> Option<Cue> {
    let opened = |was: bool, is: bool| match (was, is) {
        (false, true) => Some(Cue::TransitionUp),
        (true, false) => Some(Cue::TransitionDown),
        _ => None,
    };
    let new_notice = !before.notice && after.notice;
    match trigger {
        _ if before.get_started && !after.get_started => Some(Cue::Button),
        Trigger::Insert => Some(Cue::Type),
        Trigger::Woke if !before.listening && after.listening => Some(Cue::Notification),
        Trigger::HoldTalk if !before.talking && after.talking => Some(Cue::Button),
        Trigger::Submit if after.turns > before.turns => Some(Cue::Button),
        Trigger::Submit if new_notice => Some(Cue::Caution),
        Trigger::Submit if before.running || before.compacting => Some(Cue::Disabled),
        Trigger::Submit => None,
        Trigger::Cancel if before.running => Some(Cue::TransitionDown),
        Trigger::Cancel => None,
        Trigger::ToggleAgent => match (before.open_agent, after.open_agent) {
            (was, is) if was == is => None,
            (_, Some(_)) => Some(Cue::TransitionUp),
            (Some(_), None) => Some(Cue::TransitionDown),
            (None, None) => None,
        },
        Trigger::ToggleDraft => Some(if after.draft { Cue::ToggleOn } else { Cue::ToggleOff }),
        Trigger::SetEffort | Trigger::CompleteSkill | Trigger::ToggleGroup => Some(Cue::Select),
        Trigger::SelectModel if after.model != before.model => Some(Cue::Select),
        Trigger::SelectModel => opened(before.popover.is_some(), after.popover.is_some()),
        Trigger::OpenPopover | Trigger::ClosePopover => opened(before.popover.is_some(), after.popover.is_some()),
        Trigger::OpenBrowser if new_notice => Some(Cue::Disabled),
        Trigger::OpenBrowser => opened(before.popover.is_some(), after.popover.is_some()),
        Trigger::Add if after.attachments > before.attachments => Some(Cue::Tap),
        Trigger::Add => None,
        Trigger::Remove if after.removable() < before.removable() => Some(Cue::Swipe),
        Trigger::Remove => None,
        Trigger::ToggleReference => opened(before.expanded_reference.is_some(), after.expanded_reference.is_some()),
        Trigger::OpenWorkspace if after.root != before.root => Some(Cue::TransitionUp),
        Trigger::OpenWorkspace => None,
        Trigger::ToggleSteps => match after.open_steps.cmp(&before.open_steps) {
            std::cmp::Ordering::Greater => Some(Cue::TransitionUp),
            std::cmp::Ordering::Less => Some(Cue::TransitionDown),
            std::cmp::Ordering::Equal => None,
        },
        Trigger::DismissNotice if before.notice => Some(Cue::Swipe),
        Trigger::DismissNotice => None,
        Trigger::NewConversation if after.active != before.active => Some(Cue::TransitionUp),
        Trigger::SelectConversation if after.active != before.active => Some(Cue::Select),
        Trigger::NewConversation | Trigger::SelectConversation => None,
        Trigger::Restore if after.archived < before.archived => Some(Cue::TransitionUp),
        Trigger::Restore => None,
        Trigger::StepConversation if after.active != before.active => Some(Cue::Swipe),
        Trigger::StepConversation => Some(Cue::Disabled),
        Trigger::ScrollConversation if after.active != before.active => Some(Cue::Swipe),
        Trigger::ScrollConversation => None,
        Trigger::ChooseSetting if after.preferences != before.preferences => Some(Cue::Select),
        Trigger::ChooseSetting => None,
        Trigger::ToggleSettings => opened(before.settings.open, after.settings.open),
        Trigger::Routines if before.routines_open != after.routines_open => {
            opened(before.routines_open, after.routines_open)
        }
        Trigger::Routines if before.routine_form != after.routine_form => {
            opened(before.routine_form, after.routine_form)
        }
        Trigger::Routines if before.routine_report != after.routine_report => Some(Cue::Select),
        Trigger::Routines => None,
        Trigger::Press => Some(Cue::Button),
        Trigger::Discard => Some(Cue::Swipe),
        Trigger::SettingsPane if after.settings.pane != before.settings.pane => Some(Cue::Select),
        Trigger::SettingsPane => None,
        Trigger::CompactContext if after.compacting && !before.compacting => Some(Cue::Button),
        Trigger::CompactContext => Some(Cue::Disabled),
        Trigger::RunCompleted if before.compacting && !after.compacting => Some(Cue::Notification),
        Trigger::RunCompleted if after.runs < before.runs => {
            Some(if after.agents > 0 { Cue::Celebration } else { Cue::Notification })
        }
        Trigger::RunCompleted | Trigger::RunCancelled => None,
        Trigger::RunFailed if after.runs < before.runs => Some(Cue::Caution),
        Trigger::RunFailed => None,
        Trigger::Diagnosed | Trigger::ServerError if new_notice => Some(Cue::Caution),
        Trigger::Diagnosed | Trigger::ServerError => None,
        Trigger::WorkspaceRefreshed if before.branch.is_some() && after.branch != before.branch => Some(Cue::Select),
        Trigger::RunCommand => Some(Cue::Button),
        Trigger::ToggleMemory => opened(before.memory_open, after.memory_open),
        Trigger::Remember if before.memory_draft && !after.memory_draft => Some(Cue::Tap),
        Trigger::Remember => Some(Cue::Disabled),
        Trigger::Forget if after.memories < before.memories => Some(Cue::Swipe),
        Trigger::Asked if after.asks > before.asks => Some(Cue::Notification),
        Trigger::Answer if after.asks < before.asks => Some(Cue::Button),
        Trigger::Answer if after.picks != before.picks => Some(Cue::Select),
        Trigger::WorkspaceRefreshed
        | Trigger::Forget
        | Trigger::Asked
        | Trigger::Answer
        | Trigger::Silent
        | Trigger::Woke
        | Trigger::HoldTalk => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{Effort, RunId};
    use superfik_protocol::ClientCommand;

    use super::*;
    use crate::state::{Effect, Toward};

    fn session() -> Session {
        Session::new(Some(PathBuf::from("/repo")))
    }

    fn sound(session: &mut Session, action: Action) -> Option<Cue> {
        session.apply(action, 1.0).into_iter().find_map(|e| match e {
            Effect::Sound(cue) => Some(cue),
            _ => None,
        })
    }

    fn start_run(session: &mut Session) -> RunId {
        session.apply(Action::Edit(EditKey::Insert("Hello".into())), 0.0);
        session
            .apply(Action::Submit, 1.0)
            .into_iter()
            .find_map(|e| match e {
                Effect::Command(command) => match *command {
                    ClientCommand::SubmitPrompt { run, .. } => Some(run),
                    _ => None,
                },
                _ => None,
            })
            .expect("a run starts")
    }

    fn run_event(run: RunId, event: EngineEvent) -> Action {
        Action::Server(ServerEvent::Run { run, event })
    }

    #[test]
    fn typing_and_sending() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::Edit(EditKey::Insert("a".into()))), Some(Cue::Type));
        assert_eq!(sound(&mut s, Action::Edit(EditKey::Backspace)), None);
        assert_eq!(sound(&mut s, Action::Submit), None, "empty prompt is silent");
        start_run(&mut s);
        assert_eq!(sound(&mut s, Action::Submit), Some(Cue::Disabled), "busy");
        assert_eq!(sound(&mut s, Action::Cancel), Some(Cue::TransitionDown));
    }

    #[test]
    fn sending_plays_the_button() {
        let mut s = session();
        s.apply(Action::Edit(EditKey::Insert("Hello".into())), 0.0);
        assert_eq!(sound(&mut s, Action::Submit), Some(Cue::Button));
    }

    #[test]
    fn sending_without_a_repository_cautions() {
        let mut s = Session::new(None);
        s.apply(Action::Edit(EditKey::Insert("Hello".into())), 0.0);
        assert_eq!(sound(&mut s, Action::Submit), Some(Cue::Caution));
    }

    #[test]
    fn runs_end_with_notification_celebration_or_caution() {
        let mut s = session();
        let run = start_run(&mut s);
        assert_eq!(sound(&mut s, run_event(run, EngineEvent::TextDelta("hi".into()))), None);
        let done = EngineEvent::Completed { duration_ms: 10, cost_usd: None };
        assert_eq!(sound(&mut s, run_event(run, done.clone())), Some(Cue::Notification));
        assert_eq!(sound(&mut s, run_event(run, done.clone())), None, "stale events are silent");

        let mut s = session();
        let run = start_run(&mut s);
        let spawned = EngineEvent::AgentSpawned {
            index: 0,
            kind: "Explore".into(),
            summary: String::new(),
            prompt: String::new(),
        };
        s.apply(run_event(run, spawned), 1.0);
        assert_eq!(sound(&mut s, run_event(run, done)), Some(Cue::Celebration));

        let mut s = session();
        let run = start_run(&mut s);
        assert_eq!(sound(&mut s, run_event(run, EngineEvent::Failed("boom".into()))), Some(Cue::Caution));

        let mut s = session();
        let run = start_run(&mut s);
        assert_eq!(sound(&mut s, run_event(run, EngineEvent::Cancelled)), None, "cancel already sounded");
    }

    #[test]
    fn errors_caution_once_and_dismissing_swipes() {
        let mut s = session();
        let error = || Action::Server(ServerEvent::Error { message: "no".into() });
        assert_eq!(sound(&mut s, error()), Some(Cue::Caution));
        assert_eq!(sound(&mut s, error()), None, "a notice already showing stays quiet");
        assert_eq!(sound(&mut s, Action::DismissNotice), Some(Cue::Swipe));
        assert_eq!(sound(&mut s, Action::DismissNotice), None);
    }

    #[test]
    fn toggles_and_popovers() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::OpenPopover(PopoverKind::Model)), Some(Cue::TransitionUp));
        assert_eq!(sound(&mut s, Action::OpenPopover(PopoverKind::Branch)), None, "switching stays open");
        assert_eq!(sound(&mut s, Action::ClosePopover), Some(Cue::TransitionDown));
        assert_eq!(sound(&mut s, Action::ClosePopover), None);
    }

    #[test]
    fn choices_select_and_closing_the_picker_unchanged_transitions() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::SetEffort(Effort::High)), Some(Cue::Select));
        s.apply(Action::OpenPopover(PopoverKind::Model), 0.0);
        assert_eq!(sound(&mut s, Action::SelectModel(1)), Some(Cue::Select));
        s.apply(Action::OpenPopover(PopoverKind::Model), 0.0);
        assert_eq!(sound(&mut s, Action::SelectModel(1)), Some(Cue::TransitionDown));
    }

    #[test]
    fn attachments_tap_in_and_swipe_out() {
        let mut s = session();
        let paths = vec![PathBuf::from("/a.png")];
        assert_eq!(sound(&mut s, Action::AddReferences(paths.clone())), Some(Cue::Tap));
        assert_eq!(sound(&mut s, Action::AddReferences(paths)), None, "duplicate");
        assert_eq!(sound(&mut s, Action::ToggleReference(0)), Some(Cue::TransitionUp));
        assert_eq!(sound(&mut s, Action::ToggleReference(0)), Some(Cue::TransitionDown));
        assert_eq!(sound(&mut s, Action::RemoveReference(0)), Some(Cue::Swipe));
        assert_eq!(sound(&mut s, Action::RemoveReferences), None, "nothing left");
        s.conversation_mut().research_items.push(superfik_core::ResearchItem {
            title: "Night skies".into(),
            url: None,
            kind: superfik_core::ResearchKind::Article,
        });
        assert_eq!(sound(&mut s, Action::ClearResearch), Some(Cue::Swipe));
        assert_eq!(sound(&mut s, Action::ClearResearch), None, "nothing left");
    }

    #[test]
    fn agent_sessions_open_up_and_close_down() {
        let mut s = session();
        let run = start_run(&mut s);
        let spawned = EngineEvent::AgentSpawned {
            index: 0,
            kind: "Explore".into(),
            summary: String::new(),
            prompt: String::new(),
        };
        s.apply(run_event(run, spawned), 1.0);
        assert_eq!(sound(&mut s, Action::ToggleAgent(0)), Some(Cue::TransitionUp));
        assert_eq!(sound(&mut s, Action::CloseAgent), Some(Cue::TransitionDown));
        assert_eq!(sound(&mut s, Action::CloseAgent), None);
    }

    #[test]
    fn workspaces_and_conversations() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::OpenWorkspace(PathBuf::from("/other"))), Some(Cue::TransitionUp));
        assert_eq!(sound(&mut s, Action::NewConversation), Some(Cue::TransitionUp), "always a fresh one");
        start_run(&mut s);
        assert_eq!(sound(&mut s, Action::NewConversation), Some(Cue::TransitionUp));
        let first = s.conversations[0].id;
        assert_eq!(sound(&mut s, Action::SelectConversation(first)), Some(Cue::Select));
        assert_eq!(sound(&mut s, Action::SelectConversation(first)), None);
    }

    #[test]
    fn the_archive_group_folds_with_a_select_and_restoring_rises() {
        let mut s = session();
        start_run(&mut s);
        let id = s.active;
        s.apply(Action::ArchiveConversation(id), 1.0);
        assert_eq!(sound(&mut s, Action::ToggleArchived), Some(Cue::Select));
        assert_eq!(sound(&mut s, Action::UnarchiveConversation(id)), Some(Cue::TransitionUp));
        assert_eq!(sound(&mut s, Action::UnarchiveConversation(id)), None, "already restored");
    }

    #[test]
    fn a_browser_card_refused_for_want_of_web_pages_sounds_disabled() {
        let mut s = Session { platform: super::super::Platform::WINDOWS, ..session() };
        assert_eq!(sound(&mut s, Action::OpenBrowser), Some(Cue::Disabled));
    }

    #[test]
    fn stepping_sessions_swipes_and_a_lone_one_is_disabled() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::StepConversation(Toward::Older)), Some(Cue::Disabled), "nowhere to go");
        start_run(&mut s);
        s.apply(Action::NewConversation, 1.0);
        s.apply(Action::Edit(EditKey::Insert("Later".into())), 5.0);
        s.apply(Action::Submit, 5.0);
        assert_eq!(sound(&mut s, Action::StepConversation(Toward::Newer)), Some(Cue::Swipe), "wraps around");
    }

    #[test]
    fn scrolling_sessions_swipes_and_is_quiet_at_the_ends() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::ScrollConversation(Toward::Older)), None, "momentum past the end is quiet");
        start_run(&mut s);
        s.apply(Action::NewConversation, 1.0);
        s.apply(Action::Edit(EditKey::Insert("Later".into())), 5.0);
        s.apply(Action::Submit, 5.0);
        assert_eq!(sound(&mut s, Action::ScrollConversation(Toward::Older)), Some(Cue::Swipe));
    }

    #[test]
    fn the_settings_screen_opens_and_closes_like_a_panel() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::OpenSettings), Some(Cue::TransitionUp));
        assert_eq!(sound(&mut s, Action::OpenSettings), None, "already open: only brought forward");
        assert_eq!(sound(&mut s, Action::ShowSettingsPane(super::super::SettingsPane::Sound)), Some(Cue::Select));
        assert_eq!(sound(&mut s, Action::ShowSettingsPane(super::super::SettingsPane::Sound)), None);
        assert_eq!(sound(&mut s, Action::CloseSettings), Some(Cue::TransitionDown));
    }

    #[test]
    fn silent_actions_stay_silent() {
        let mut s = session();
        assert_eq!(sound(&mut s, Action::RequestWorkspace), None);
        assert_eq!(sound(&mut s, Action::RestoreRecents(Vec::new())), None);
        assert_eq!(sound(&mut s, Action::Server(ServerEvent::Skills(Vec::new()))), None);
    }
}
