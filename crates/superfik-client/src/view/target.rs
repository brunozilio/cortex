//! Clickable things on the canvas and the per-frame hit map.

use superfik_core::ConversationId;
use superfik_render::{Point, Rect};
use superfik_ui::components::{BrowserPart, ComposerPart};

use super::layout::NodeKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Composer(ComposerPart),
    Input,
    Agent(usize),
    CloseAgent,
    RemoveWorkspace,
    Branch,
    BranchItem(usize),
    AddRepository,
    AttachRepositoryItem(usize),
    ChooseAttachedRepository,
    RemoveRepository(usize),
    PullRequests,
    OpenPullRequest,
    Review(ReviewTarget),
    OpenPlugins,
    DelegateReview,
    OpenReview,
    PullRequestField(crate::state::PrField),
    WritePullRequestWithAgent,
    TogglePullRequestDraft,
    ConfirmPullRequest,
    CancelPullRequest,
    PullRequestItem(usize),
    ModelItem(usize),
    SkillItem(usize),
    Command(usize),
    AddSkill,
    AddSkillItem(usize),
    RemoveSkill(usize),
    Page(crate::state::Carousel, i32),
    PluginRow(usize),
    AccountChoice(usize),
    AddReference,
    Reference(usize),
    CompactContext,
    Scrim,
    CloseViewer,
    RemoveReference(usize),
    Close(NodeKey),
    RepositoryItem(usize),
    ChooseRepository,
    GitHubRepositoryItem(usize),
    AttachGitHubRepositoryItem(usize),
    SignInToGitHub,
    Node(NodeKey),
    Steps(usize),
    RetryTurn(usize),
    VisualPrompt(crate::state::VisualPrompt),
    StepOutput(usize, usize),
    RecentreComposer,
    DismissNotice,
    Surface,
    Conversation(ConversationId),
    NewConversation,
    ArchiveConversation(ConversationId),
    DayGroup(i64),
    ArchivedGroup,
    ArchivedConversation(ConversationId),
    UnarchiveConversation(ConversationId),
    OpenSettings,
    Settings(SettingsTarget),
    OpenRoutines,
    Routines(RoutinesTarget),
    Memory(MemoryTarget),
    Note(NoteTarget),
    Ask(AskTarget),
    PermissionItem(superfik_core::PermissionMode),
    PasskeyItem(usize),
    PasskeyElsewhere,
    GetStarted(GetStartedTarget),
    Lock(LockTarget),
    Update(UpdateTarget),
    LoginItem(usize),
    SaveLoginItem(usize),
    DismissSaveLogin,
    Browser(superfik_ui::components::BrowserPart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutinesTarget {
    Surface,
    Back,
    New,
    RunNow(superfik_core::routine::RoutineId),
    Toggle(superfik_core::routine::RoutineId),
    Revise(superfik_core::routine::RoutineId),
    Delete(superfik_core::routine::RoutineId),
    Field(crate::state::routines::RoutineField),
    Cadence(crate::state::routines::Cadence),
    Autonomy(superfik_core::routine::Autonomy),
    Template(superfik_core::routine::Template),
    Account(superfik_core::ConnectionId),
    Answers,
    Webhook,
    CopyWebhook(superfik_core::routine::RoutineId),
    ApproveDraft(superfik_core::ConnectionId, u32),
    DiscardDraft(superfik_core::ConnectionId, u32),
    EditDraft(superfik_core::ConnectionId, u32),
    DraftField,
    Save,
    Cancel,
    Report(superfik_core::RunId),
    Continue(superfik_core::RunId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTarget {
    Profile(ProfilesTarget),
    Surface,
    Back,
    Pane(crate::state::SettingsPane),
    Choose(crate::state::Setting),
    Run(crate::view::settings_screen::rows::Run),
    Field(crate::view::settings_screen::rows::Field),
    Plugin(PluginsTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilesTarget {
    New,
    Edit(superfik_core::browser_profiles::ProfileId),
    Delete(superfik_core::browser_profiles::ProfileId),
    Assign(Option<superfik_core::browser_profiles::ProfileId>),
    Field(superfik_core::browser_profiles::ProfileField),
    Toggle(crate::state::browser_profiles::ProfileToggle),
    WebRtc(superfik_core::browser_profiles::WebRtc),
    Save,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTarget {
    Primary,
    Close,
    Open,
    Surface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockTarget {
    Unlock,
    Surface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetStartedTarget {
    Continue,
    Surface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskTarget {
    Option(usize),
    Reply(crate::state::Reply),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteTarget {
    Card(superfik_core::note::NoteId),
    Check(superfik_core::note::NoteId, usize),
    Unlink(superfik_core::note::NoteId),
    Link(superfik_core::note::NoteId),
    Draft,
    Paper(usize),
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTarget {
    Search,
    Open,
    Item(superfik_core::MemoryId),
    Forget(superfik_core::MemoryId),
    Share(superfik_core::MemoryId),
    Revise(superfik_core::MemoryId),
    Revision,
    RevisionKind(superfik_core::MemoryKind),
    Kind(superfik_core::MemoryKind),
    Draft,
    Remember,
    Close,
    Optimize,
    Undo,
    Graph,
    Surface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginsTarget {
    AddFolder,
    Get(usize),
    AiConnect(usize),
    LocalModels,
    LocalModelField,
    DownloadModel,
    LocalModelPicker,
    LocalModelChoice(usize),
    Open(usize),
    Fold,
    Uninstall,
    AddAccount,
    Disconnect(superfik_core::ConnectionId),
    CopyHook(superfik_core::ConnectionId),
    Field(usize),
    Connect,
    CancelSheet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewTarget {
    Back,
    File(usize),
    Layout(crate::state::DiffLayout),
    Line { side: superfik_core::CommentSide, line: u32 },
    AddComment,
    AskAgent,
    Approve,
    RequestChanges,
    Knob { comment: u64, knob: superfik_ui::components::AnnotationKnob },
    Outgoing(u64),
    Publish,
    CancelPublish,
    Surface,
}

impl Target {
    pub fn is_button(self) -> bool {
        !matches!(
            self,
            Self::Input
                | Self::Surface
                | Self::Node(_)
                | Self::Scrim
                | Self::Browser(BrowserPart::Address | BrowserPart::Page)
                | Self::Review(ReviewTarget::Surface)
                | Self::Memory(
                    MemoryTarget::Search
                        | MemoryTarget::Draft
                        | MemoryTarget::Revision
                        | MemoryTarget::Graph
                        | MemoryTarget::Surface
                )
        )
    }
}

#[derive(Debug, Default, Clone)]
pub struct HitMap {
    entries: Vec<(Rect, Target)>,
    screen: Vec<(Rect, Target)>,
    pub effort_track: Option<Rect>,
    pub input_scroll: Option<(f32, bool)>,
    pub input_text: Option<(std::sync::Arc<superfik_render::TextLayout>, Point)>,
    pub viewed_tile: Option<Rect>,
    pub memory_node: Option<Rect>,
    pub repository_node: Option<Rect>,
    pub carousels: Vec<(Rect, crate::state::Carousel)>,
    pub open_agent_node: Option<Rect>,
    pub memory_list: Option<(Rect, f32)>,
    pub agent_sheet: Option<(Rect, f32)>,
    pub memory_graph: Option<Rect>,
    pub review_panes: Option<(Rect, Rect)>,
    pub settings_scroll: Option<(Rect, f32)>,
    pub model_scroll: Option<(Rect, f32)>,
    pub routines_scroll: Option<(Rect, f32)>,
    pub browser_page: Option<(superfik_core::BrowserId, Rect)>,
    pub browser_card: Option<Rect>,
    pub expanded_page: Option<(superfik_core::BrowserId, Rect)>,
    pub attached: Vec<(Option<crate::view::layout::NodeKey>, Rect)>,
}

impl HitMap {
    pub fn add(&mut self, rect: Rect, target: Target) {
        if rect.w > 0.0 && rect.h > 0.0 {
            self.entries.push((rect, target));
        }
    }

    pub fn add_screen(&mut self, rect: Rect, target: Target) {
        if rect.w > 0.0 && rect.h > 0.0 {
            self.screen.push((rect, target));
        }
    }

    pub fn at(&self, screen: Point, canvas: Point) -> Option<Target> {
        let hit = |entries: &[(Rect, Target)], point: Point| {
            entries.iter().rev().find(|(rect, _)| rect.contains(point)).map(|(_, target)| *target)
        };
        hit(&self.screen, screen).or_else(|| hit(&self.entries, canvas))
    }

    pub fn on_screen(&self, screen: Point) -> bool {
        self.screen.iter().any(|(rect, _)| rect.contains(screen))
    }

    pub fn screen_len(&self) -> usize {
        self.screen.len()
    }

    pub fn screen_covers_from(&self, from: usize, screen: Point) -> bool {
        self.screen.iter().skip(from).any(|(rect, _)| rect.contains(screen))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topmost_entry_wins() {
        let mut hits = HitMap::default();
        hits.add(Rect::new(0.0, 0.0, 100.0, 100.0), Target::Surface);
        hits.add(Rect::new(10.0, 10.0, 10.0, 10.0), Target::AddReference);
        let at = |x, y| hits.at(Point::new(x, y), Point::new(x, y));
        assert_eq!(at(15.0, 15.0), Some(Target::AddReference));
        assert_eq!(at(50.0, 50.0), Some(Target::Surface));
        assert_eq!(at(150.0, 50.0), None);
    }

    #[test]
    fn screen_entries_shadow_canvas_entries() {
        let mut hits = HitMap::default();
        hits.add(Rect::new(0.0, 0.0, 100.0, 100.0), Target::Surface);
        hits.add_screen(Rect::new(0.0, 0.0, 40.0, 100.0), Target::NewConversation);
        assert_eq!(hits.at(Point::new(10.0, 10.0), Point::new(90.0, 90.0)), Some(Target::NewConversation));
        assert_eq!(hits.at(Point::new(60.0, 10.0), Point::new(90.0, 90.0)), Some(Target::Surface));
        assert!(hits.on_screen(Point::new(10.0, 10.0)));
        assert!(!hits.on_screen(Point::new(60.0, 10.0)));
    }

    #[test]
    fn empty_rects_are_ignored() {
        let mut hits = HitMap::default();
        hits.add(Rect::new(0.0, 0.0, 0.0, 10.0), Target::Surface);
        assert_eq!(hits.at(Point::new(0.0, 5.0), Point::new(0.0, 5.0)), None);
    }
}
