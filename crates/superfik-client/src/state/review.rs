//! The review screen: a pull request's files and diff over the canvas,
//! comments drafted on it (by the user or an agent), and the review that
//! publishes them. Drafts survive closing the screen; nothing reaches
//! GitHub without `PublishReview`.

use superfik_core::{
    CommentOrigin, CommentSide, CommentState, PullRequestDetail, ReviewComment, ReviewCommentDraft, ReviewVerdict,
};
use superfik_protocol::ClientCommand;
use superfik_ui::TextEdit;

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffLayout {
    #[default]
    Stacked,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSelection {
    pub side: CommentSide,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentEditor {
    pub comment: Option<u64>,
    pub path: String,
    pub side: CommentSide,
    pub line: u32,
    pub body: TextEdit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishSheet {
    pub verdict: ReviewVerdict,
    pub summary: TextEdit,
    pub skipped: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewScreen {
    pub number: u64,
    pub detail: Option<PullRequestDetail>,
    pub selected_file: usize,
    pub layout: DiffLayout,
    pub selection: Option<LineSelection>,
    pub drafts: Vec<ReviewComment>,
    pub editor: Option<CommentEditor>,
    pub publish: Option<PublishSheet>,
    pub publishing: bool,
    pub files_scroll: f32,
    pub diff_scroll: f32,
    pub diff_scroll_x: f32,
    pub open: bool,
    next_id: u64,
}

impl ReviewScreen {
    fn new(number: u64) -> Self {
        Self {
            number,
            detail: None,
            selected_file: 0,
            layout: DiffLayout::Stacked,
            selection: None,
            drafts: Vec::new(),
            editor: None,
            publish: None,
            publishing: false,
            files_scroll: 0.0,
            diff_scroll: 0.0,
            diff_scroll_x: 0.0,
            open: true,
            next_id: 1,
        }
    }

    pub fn selected_path(&self) -> Option<&str> {
        self.detail.as_ref()?.files.get(self.selected_file).map(|f| f.path())
    }

    pub fn comments_on<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a ReviewComment> + 'a {
        let published = self.detail.iter().flat_map(|d| d.comments.iter());
        published.chain(self.drafts.iter()).filter(move |c| c.path == path)
    }

    pub fn outgoing(&self) -> Vec<ReviewCommentDraft> {
        let skipped = self.publish.as_ref().map(|p| p.skipped.as_slice()).unwrap_or_default();
        self.drafts
            .iter()
            .filter(|c| !skipped.contains(&c.id))
            .map(|c| ReviewCommentDraft { path: c.path.clone(), line: c.line, side: c.side, body: c.body.clone() })
            .collect()
    }

    fn allocate(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_agent_drafts(&mut self, comments: Vec<ReviewCommentDraft>) -> usize {
        let count = comments.len();
        for c in comments {
            let id = self.allocate();
            self.drafts.push(ReviewComment {
                id,
                path: c.path,
                line: c.line,
                side: c.side,
                body: c.body,
                author: String::new(),
                origin: CommentOrigin::Agent,
                state: CommentState::Draft,
                thread: None,
            });
        }
        count
    }
}

pub fn ask_prompt(path: &str, selection: LineSelection, excerpt: &[String]) -> String {
    let lines = if selection.from == selection.to {
        format!("line {}", selection.from)
    } else {
        format!("lines {}–{}", selection.from, selection.to)
    };
    let side = match selection.side {
        CommentSide::Old => " (old version)",
        CommentSide::New => "",
    };
    let mut prompt = format!("About {path} {lines}{side}:\n");
    for line in excerpt {
        prompt.push_str("    ");
        prompt.push_str(line);
        prompt.push('\n');
    }
    prompt.push_str("Explain this change and whether it looks right.");
    prompt
}

impl Session {
    pub(super) fn open_review(&mut self) -> Vec<Effect> {
        let Some(number) = self.conversation().pull_request.as_ref().map(|pr| pr.number) else { return Vec::new() };
        let Some(path) = self.workspace_root() else { return Vec::new() };
        self.popover = None;
        let fresh = self.review.as_ref().is_none_or(|r| r.number != number);
        if fresh {
            self.review = Some(ReviewScreen::new(number));
        }
        let review = self.review.as_mut().expect("just set");
        review.open = true;
        if review.detail.is_some() {
            return Vec::new();
        }
        self.notice = Some(super::Notice::LoadingPullRequest(number));
        vec![Effect::Command(Box::new(ClientCommand::LoadPullRequest { path, number }))]
    }

    pub(super) fn close_review(&mut self) -> Vec<Effect> {
        if let Some(review) = self.review.as_mut() {
            review.open = false;
            review.editor = None;
            review.publish = None;
        }
        Vec::new()
    }

    pub fn review_open(&self) -> bool {
        self.review.as_ref().is_some_and(|r| r.open)
    }

    pub(super) fn pull_request_loaded(&mut self, detail: PullRequestDetail) {
        if matches!(self.notice, Some(super::Notice::LoadingPullRequest(_))) {
            self.notice = None;
        }
        let number = detail.pull_request.number;
        let review = self.review.get_or_insert_with(|| ReviewScreen::new(number));
        if review.number != number {
            return;
        }
        review.selected_file = review.selected_file.min(detail.files.len().saturating_sub(1));
        review.detail = Some(detail);
    }

    fn review_mut(&mut self) -> Option<&mut ReviewScreen> {
        self.review.as_mut().filter(|r| r.open)
    }

    pub(super) fn select_review_file(&mut self, index: usize) -> Vec<Effect> {
        if let Some(review) = self.review_mut()
            && review.detail.as_ref().is_some_and(|d| index < d.files.len())
        {
            review.selected_file = index;
            review.selection = None;
            review.editor = None;
            review.diff_scroll = 0.0;
            review.diff_scroll_x = 0.0;
        }
        Vec::new()
    }

    pub(super) fn set_diff_layout(&mut self, layout: DiffLayout) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.layout = layout;
        }
        Vec::new()
    }

    pub(super) fn select_diff_line(&mut self, side: CommentSide, line: u32, extend: bool) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.editor = None;
            review.selection = match (extend, review.selection) {
                (true, Some(current)) if current.side == side => {
                    Some(LineSelection { side, from: current.from.min(line), to: current.to.max(line) })
                }
                _ => Some(LineSelection { side, from: line, to: line }),
            };
        }
        Vec::new()
    }

    pub(super) fn scroll_review(&mut self, files: f32, diff: f32, diff_x: f32) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.files_scroll = (review.files_scroll + files).max(0.0);
            review.diff_scroll = (review.diff_scroll + diff).max(0.0);
            review.diff_scroll_x = (review.diff_scroll_x + diff_x).max(0.0);
        }
        Vec::new()
    }

    pub(super) fn add_review_comment(&mut self) -> Vec<Effect> {
        if let Some(review) = self.review_mut()
            && let (Some(selection), Some(path)) = (review.selection, review.selected_path().map(str::to_owned))
        {
            review.editor = Some(CommentEditor {
                comment: None,
                path,
                side: selection.side,
                line: selection.to,
                body: TextEdit::default(),
            });
        }
        Vec::new()
    }

    pub(super) fn edit_review_comment(&mut self, id: u64) -> Vec<Effect> {
        if let Some(review) = self.review_mut()
            && let Some(draft) = review.drafts.iter().find(|c| c.id == id)
        {
            review.editor = Some(CommentEditor {
                comment: Some(id),
                path: draft.path.clone(),
                side: draft.side,
                line: draft.line,
                body: TextEdit::new(draft.body.clone()),
            });
        }
        Vec::new()
    }

    pub(super) fn save_review_comment(&mut self) -> Vec<Effect> {
        let Some(review) = self.review_mut() else { return Vec::new() };
        let Some(editor) = review.editor.take() else { return Vec::new() };
        let body = editor.body.text().trim().to_owned();
        if body.is_empty() {
            return Vec::new();
        }
        match editor.comment.and_then(|id| review.drafts.iter_mut().find(|c| c.id == id)) {
            Some(draft) => draft.body = body,
            None => {
                let id = review.allocate();
                review.drafts.push(ReviewComment {
                    id,
                    path: editor.path,
                    line: editor.line,
                    side: editor.side,
                    body,
                    author: String::new(),
                    origin: CommentOrigin::User,
                    state: CommentState::Draft,
                    thread: None,
                });
            }
        }
        Vec::new()
    }

    pub(super) fn cancel_review_comment(&mut self) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.editor = None;
        }
        Vec::new()
    }

    pub(super) fn delete_review_comment(&mut self, id: u64) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.drafts.retain(|c| c.id != id);
            if review.editor.as_ref().is_some_and(|e| e.comment == Some(id)) {
                review.editor = None;
            }
        }
        Vec::new()
    }

    pub(super) fn resolve_review_thread(&mut self, id: u64, resolved: bool) -> Vec<Effect> {
        let Some(path) = self.workspace_root() else { return Vec::new() };
        let Some(review) = self.review_mut() else { return Vec::new() };
        let thread =
            review.detail.as_ref().and_then(|d| d.comments.iter().find(|c| c.id == id)).and_then(|c| c.thread.clone());
        match thread {
            Some(thread) => vec![Effect::Command(Box::new(ClientCommand::ResolveThread { path, thread, resolved }))],
            None => Vec::new(),
        }
    }

    pub(super) fn thread_resolved(&mut self, thread: &str, resolved: bool) {
        if let Some(detail) = self.review.as_mut().and_then(|r| r.detail.as_mut()) {
            for comment in detail.comments.iter_mut().filter(|c| c.thread.as_deref() == Some(thread)) {
                comment.state = if resolved { CommentState::Resolved } else { CommentState::Published };
            }
        }
    }

    pub(super) fn ask_about_selection(&mut self) -> Vec<Effect> {
        let Some(review) = self.review.as_ref().filter(|r| r.open) else { return Vec::new() };
        let (Some(selection), Some(detail)) = (review.selection, review.detail.as_ref()) else { return Vec::new() };
        let Some(file) = detail.files.get(review.selected_file) else { return Vec::new() };
        let excerpt: Vec<String> = file
            .hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| {
                let n = match selection.side {
                    CommentSide::Old => l.old_no,
                    CommentSide::New => l.new_no,
                };
                n.is_some_and(|n| (selection.from..=selection.to).contains(&n))
                    && (selection.side == CommentSide::New) != (l.kind == superfik_core::LineKind::Removed)
            })
            .map(|l| l.text.clone())
            .collect();
        let prompt = ask_prompt(file.path(), selection, &excerpt);
        self.conversation_mut().composer.set(prompt);
        self.close_review()
    }

    pub(super) fn begin_publish_review(&mut self, verdict: ReviewVerdict) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.editor = None;
            review.publish = Some(PublishSheet { verdict, summary: TextEdit::default(), skipped: Vec::new() });
        }
        Vec::new()
    }

    pub(super) fn toggle_outgoing_comment(&mut self, id: u64) -> Vec<Effect> {
        if let Some(publish) = self.review_mut().and_then(|r| r.publish.as_mut()) {
            match publish.skipped.iter().position(|s| *s == id) {
                Some(i) => {
                    publish.skipped.remove(i);
                }
                None => publish.skipped.push(id),
            }
        }
        Vec::new()
    }

    pub(super) fn cancel_publish_review(&mut self) -> Vec<Effect> {
        if let Some(review) = self.review_mut() {
            review.publish = None;
        }
        Vec::new()
    }

    pub(super) fn publish_review(&mut self) -> Vec<Effect> {
        let Some(path) = self.workspace_root() else { return Vec::new() };
        let Some(review) = self.review_mut() else { return Vec::new() };
        let Some(publish) = review.publish.as_ref() else { return Vec::new() };
        if review.publishing {
            return Vec::new();
        }
        let comments = review.outgoing();
        let (number, verdict, body) = (review.number, publish.verdict, publish.summary.text().trim().to_owned());
        review.publishing = true;
        self.notice = Some(super::Notice::PublishingReview);
        vec![Effect::Command(Box::new(ClientCommand::SubmitReview { path, number, verdict, body, comments }))]
    }

    pub(super) fn review_submitted(&mut self, number: u64) -> Vec<Effect> {
        let Some(path) = self.workspace_root() else { return Vec::new() };
        let Some(review) = self.review.as_mut().filter(|r| r.number == number) else { return Vec::new() };
        let skipped = review.publish.take().map(|p| p.skipped).unwrap_or_default();
        review.drafts.retain(|c| skipped.contains(&c.id));
        review.publishing = false;
        review.detail = None;
        self.notice = Some(super::Notice::ReviewPublished(number));
        vec![Effect::Command(Box::new(ClientCommand::LoadPullRequest { path, number }))]
    }

    pub(super) fn review_failed(&mut self) {
        if let Some(review) = self.review.as_mut() {
            review.publishing = false;
        }
    }

    pub(super) fn delegate_review(&mut self, now: f64) -> Vec<Effect> {
        let Some(number) = self.conversation().pull_request.as_ref().map(|pr| pr.number) else { return Vec::new() };
        if self.is_running() {
            return Vec::new();
        }
        self.conversation_mut().composer.set(format!("Review pull request #{number}."));
        self.pending_handoff = Some(superfik_core::HandoffKind::Review { number });
        let effects = self.submit(now);
        self.pending_handoff = None;
        effects
    }

    pub(super) fn review_drafted(&mut self, number: u64, comments: Vec<ReviewCommentDraft>) {
        let review = match self.review.as_mut() {
            Some(review) if review.number == number => review,
            _ => {
                let mut fresh = ReviewScreen::new(number);
                fresh.open = false;
                self.review.insert(fresh)
            }
        };
        let count = review.add_agent_drafts(comments);
        self.notice = Some(super::Notice::AgentReviewReady(count));
    }

    pub fn pending_review_comments(&self) -> usize {
        self.review.as_ref().map_or(0, |r| r.drafts.len())
    }

    pub(super) fn edit_review(&mut self, key: &super::EditKey) -> bool {
        let Some(review) = self.review_mut() else { return false };
        let edit = match (&mut review.editor, &mut review.publish) {
            (Some(editor), _) => &mut editor.body,
            (None, Some(publish)) => &mut publish.summary,
            (None, None) => return false,
        };
        super::composer::apply_key(edit, key.clone());
        true
    }

    pub(super) fn enter_review(&mut self) -> bool {
        let Some(review) = self.review_mut() else { return false };
        if let Some(editor) = review.editor.as_mut() {
            editor.body.insert("\n");
            return true;
        }
        if let Some(publish) = review.publish.as_mut() {
            publish.summary.insert("\n");
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use superfik_core::{PullRequest, Repository, WorkspaceSnapshot};
    use superfik_protocol::ServerEvent;

    use super::*;
    use crate::state::tests::commands;
    use crate::state::{Action, EditKey, Notice};

    const PATCH: &str = "diff --git a/src/sky.rs b/src/sky.rs\n--- a/src/sky.rs\n+++ b/src/sky.rs\n@@ -1,3 +1,3 @@\n use stars;\n-let moon = 1;\n+let moon = 2;\n let end = 0;\ndiff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1 @@\n-a\n+b\n";

    fn detail(number: u64) -> PullRequestDetail {
        PullRequestDetail {
            pull_request: PullRequest { number, title: "Sky".into(), ..PullRequest::default() },
            description: "Stars".into(),
            files: superfik_core::diff::parse(PATCH),
            comments: vec![ReviewComment {
                id: 900,
                path: "src/sky.rs".into(),
                line: 2,
                side: CommentSide::New,
                body: "nice".into(),
                author: "ana".into(),
                origin: CommentOrigin::User,
                state: CommentState::Published,
                thread: Some("T1".into()),
            }],
        }
    }

    fn session() -> Session {
        let mut session = Session::new(Some(PathBuf::from("/r")));
        session.snapshot = WorkspaceSnapshot {
            repository: Some(Repository { root: "/r".into(), slug: "acme/rocket".into(), ..Repository::default() }),
        };
        session.conversation_mut().pull_request = Some(PullRequest { number: 7, ..PullRequest::default() });
        session
    }

    fn opened() -> Session {
        let mut session = session();
        session.apply(Action::OpenReview, 0.0);
        session.apply(Action::Server(ServerEvent::PullRequestLoaded(Box::new(detail(7)))), 0.0);
        session
    }

    #[test]
    fn opening_loads_the_pull_request_once_and_closing_keeps_it() {
        let mut session = session();
        let effects = session.apply(Action::OpenReview, 0.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::LoadPullRequest { number: 7, .. }]));
        assert_eq!(session.notice, Some(Notice::LoadingPullRequest(7)));
        assert!(session.review_open());
        session.apply(Action::Server(ServerEvent::PullRequestLoaded(Box::new(detail(7)))), 0.0);
        assert_eq!(session.notice, None);
        assert_eq!(session.review.as_ref().and_then(|r| r.selected_path()), Some("src/sky.rs"));
        session.apply(Action::CloseReview, 0.0);
        assert!(!session.review_open());
        assert!(session.apply(Action::OpenReview, 0.0).is_empty(), "already loaded");
        assert!(session.review_open());
        assert!(Session::new(None).apply(Action::OpenReview, 0.0).is_empty());
    }

    #[test]
    fn the_diff_scrolls_both_ways_and_a_new_file_starts_at_the_origin() {
        let mut session = opened();
        session.apply(Action::ScrollReview { files: 0.0, diff: 120.0, diff_x: 80.0 }, 0.0);
        session.apply(Action::ScrollReview { files: 0.0, diff: -500.0, diff_x: -20.0 }, 0.0);
        let review = session.review.as_ref().expect("review");
        assert_eq!((review.diff_scroll, review.diff_scroll_x), (0.0, 60.0));
        session.apply(Action::SelectReviewFile(1), 0.0);
        let review = session.review.as_ref().expect("review");
        assert_eq!((review.diff_scroll, review.diff_scroll_x), (0.0, 0.0));
    }

    #[test]
    fn files_lines_and_layout_are_chosen_on_screen() {
        let mut session = opened();
        session.apply(Action::SelectReviewFile(1), 0.0);
        session.apply(Action::SelectReviewFile(9), 0.0);
        assert_eq!(session.review.as_ref().and_then(|r| r.selected_path()), Some("b.rs"));
        session.apply(Action::SetDiffLayout(DiffLayout::Split), 0.0);
        session.apply(Action::SelectDiffLine { side: CommentSide::New, line: 1, extend: false }, 0.0);
        session.apply(Action::SelectDiffLine { side: CommentSide::New, line: 3, extend: true }, 0.0);
        session.apply(Action::SelectDiffLine { side: CommentSide::Old, line: 2, extend: true }, 0.0);
        let review = session.review.as_ref().expect("review");
        assert_eq!(review.layout, DiffLayout::Split);
        assert_eq!(
            review.selection,
            Some(LineSelection { side: CommentSide::Old, from: 2, to: 2 }),
            "a new side restarts"
        );
    }

    #[test]
    fn comments_are_drafted_edited_deleted_and_published_on_confirmation() {
        let mut session = opened();
        session.apply(Action::SelectDiffLine { side: CommentSide::New, line: 2, extend: false }, 0.0);
        session.apply(Action::AddReviewComment, 0.0);
        session.apply(Action::Edit(EditKey::Insert("Use 3".into())), 0.0);
        session.apply(Action::Submit, 0.0);
        session.apply(Action::Edit(EditKey::Insert("here".into())), 0.0);
        assert!(session.conversation().composer.is_empty(), "keys went to the editor");
        session.apply(Action::SaveReviewComment, 0.0);
        let draft = session.review.as_ref().expect("review").drafts[0].clone();
        assert_eq!(
            (draft.path.as_str(), draft.line, draft.body.as_str(), draft.origin),
            ("src/sky.rs", 2, "Use 3\nhere", CommentOrigin::User)
        );
        session.apply(Action::EditReviewComment(draft.id), 0.0);
        session.apply(Action::Edit(EditKey::SelectAll), 0.0);
        session.apply(Action::Edit(EditKey::Insert("Use 4".into())), 0.0);
        session.apply(Action::SaveReviewComment, 0.0);
        assert_eq!(session.review.as_ref().expect("review").drafts[0].body, "Use 4");
        session.apply(Action::AddReviewComment, 0.0);
        session.apply(Action::SaveReviewComment, 0.0);
        assert_eq!(session.review.as_ref().expect("review").drafts.len(), 1, "empty comments are not kept");
        session.apply(Action::SelectDiffLine { side: CommentSide::Old, line: 2, extend: false }, 0.0);
        session.apply(Action::AddReviewComment, 0.0);
        session.apply(Action::Edit(EditKey::Insert("Old line".into())), 0.0);
        session.apply(Action::SaveReviewComment, 0.0);
        let second = session.review.as_ref().expect("review").drafts[1].id;
        session.apply(Action::DeleteReviewComment(draft.id), 0.0);
        assert_eq!(session.review.as_ref().expect("review").drafts.len(), 1);
        assert!(session.apply(Action::PublishReview, 0.0).is_empty(), "no sheet, no publish");
        session.apply(Action::BeginPublishReview(ReviewVerdict::RequestChanges), 0.0);
        session.apply(Action::Edit(EditKey::Insert("Almost".into())), 0.0);
        let effects = session.apply(Action::PublishReview, 0.0);
        assert!(matches!(
            commands(&effects)[..],
            [ClientCommand::SubmitReview { number: 7, verdict: ReviewVerdict::RequestChanges, body, comments, .. }]
                if body == "Almost" && comments.len() == 1 && comments[0].side == CommentSide::Old
        ));
        assert_eq!(session.notice, Some(Notice::PublishingReview));
        assert!(session.apply(Action::PublishReview, 0.0).is_empty(), "no double submit");
        let effects = session.apply(Action::Server(ServerEvent::ReviewSubmitted { number: 7 }), 0.0);
        assert!(matches!(commands(&effects)[..], [ClientCommand::LoadPullRequest { number: 7, .. }]));
        let review = session.review.as_ref().expect("review");
        assert!(review.drafts.is_empty() && review.publish.is_none() && !review.publishing);
        assert_eq!(session.notice, Some(Notice::ReviewPublished(7)));
        let _ = second;
    }

    #[test]
    fn skipped_drafts_stay_local_and_errors_keep_the_sheet() {
        let mut session = opened();
        session.apply(Action::SelectDiffLine { side: CommentSide::New, line: 1, extend: false }, 0.0);
        session.apply(Action::AddReviewComment, 0.0);
        session.apply(Action::Edit(EditKey::Insert("one".into())), 0.0);
        session.apply(Action::SaveReviewComment, 0.0);
        session.apply(Action::AddReviewComment, 0.0);
        session.apply(Action::Edit(EditKey::Insert("two".into())), 0.0);
        session.apply(Action::SaveReviewComment, 0.0);
        let first = session.review.as_ref().expect("review").drafts[0].id;
        session.apply(Action::BeginPublishReview(ReviewVerdict::Approve), 0.0);
        session.apply(Action::ToggleOutgoingComment(first), 0.0);
        let effects = session.apply(Action::PublishReview, 0.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::SubmitReview { comments, .. }] if comments.len() == 1 && comments[0].body == "two")
        );
        session.apply(Action::Server(ServerEvent::error("`gh api` failed")), 0.0);
        let review = session.review.as_ref().expect("review");
        assert!(!review.publishing && review.publish.is_some());
        session.apply(Action::PublishReview, 0.0);
        session.apply(Action::Server(ServerEvent::ReviewSubmitted { number: 7 }), 0.0);
        let review = session.review.as_ref().expect("review");
        assert_eq!(review.drafts.iter().map(|c| c.body.as_str()).collect::<Vec<_>>(), vec!["one"]);
    }

    #[test]
    fn published_threads_resolve_through_github() {
        let mut session = opened();
        let effects = session.apply(Action::ResolveReviewThread { id: 900, resolved: true }, 0.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::ResolveThread { thread, resolved: true, .. }] if thread == "T1")
        );
        session.apply(Action::Server(ServerEvent::ThreadResolved { thread: "T1".into(), resolved: true }), 0.0);
        let state = session.review.as_ref().and_then(|r| r.detail.as_ref()).map(|d| d.comments[0].state);
        assert_eq!(state, Some(CommentState::Resolved));
        assert!(
            session.apply(Action::ResolveReviewThread { id: 1, resolved: true }, 0.0).is_empty(),
            "drafts have no thread"
        );
    }

    #[test]
    fn asking_the_agent_seeds_the_composer_with_the_lines_and_returns() {
        let mut session = opened();
        session.apply(Action::SelectDiffLine { side: CommentSide::New, line: 1, extend: false }, 0.0);
        session.apply(Action::SelectDiffLine { side: CommentSide::New, line: 2, extend: true }, 0.0);
        session.apply(Action::AskAboutSelection, 0.0);
        let text = session.conversation().composer.text().to_owned();
        assert!(text.starts_with("About src/sky.rs lines 1–2:\n    use stars;\n    let moon = 2;\n"), "{text}");
        assert!(!session.review_open());
        assert!(session.review.is_some(), "drafts and detail are kept");
    }

    #[test]
    fn agent_findings_arrive_as_pending_drafts() {
        let mut review = ReviewScreen::new(7);
        let n = review.add_agent_drafts(vec![ReviewCommentDraft {
            path: "a.rs".into(),
            line: 3,
            side: CommentSide::New,
            body: "hm".into(),
        }]);
        assert_eq!(n, 1);
        assert!(review.drafts[0].origin == CommentOrigin::Agent && review.drafts[0].is_draft());
        assert_eq!(review.outgoing().len(), 1);
    }

    #[test]
    fn delegating_runs_a_review_handoff_and_its_drafts_wait_for_approval() {
        let mut session = session();
        let effects = session.apply(Action::DelegateReview, 0.0);
        let [ClientCommand::SubmitPrompt { request, .. }] = commands(&effects)[..] else { panic!("expected a submit") };
        assert_eq!(
            request.handoff.as_ref().map(|h| h.kind.clone()),
            Some(superfik_core::HandoffKind::Review { number: 7 })
        );
        assert!(session.apply(Action::DelegateReview, 0.0).is_empty(), "one run at a time");
        let comments = vec![
            ReviewCommentDraft { path: "src/sky.rs".into(), line: 2, side: CommentSide::New, body: "Use 3".into() },
            ReviewCommentDraft { path: "b.rs".into(), line: 1, side: CommentSide::New, body: "nit: b".into() },
        ];
        session.apply(Action::Server(ServerEvent::ReviewDrafted { number: 7, summary: "ok".into(), comments }), 0.0);
        assert_eq!(session.notice, Some(Notice::AgentReviewReady(2)));
        assert_eq!(session.pending_review_comments(), 2);
        assert!(!session.review_open(), "the screen opens when the user asks");
        session.conversation_mut().run = None;
        session.apply(Action::OpenReview, 0.0);
        session.apply(Action::Server(ServerEvent::PullRequestLoaded(Box::new(detail(7)))), 0.0);
        let first = session.review.as_ref().expect("review").drafts[0].id;
        session.apply(Action::DeleteReviewComment(first), 0.0);
        session.apply(Action::BeginPublishReview(ReviewVerdict::Comment), 0.0);
        let effects = session.apply(Action::PublishReview, 0.0);
        assert!(
            matches!(commands(&effects)[..], [ClientCommand::SubmitReview { comments, .. }] if comments.len() == 1 && comments[0].body == "nit: b")
        );
    }
}
