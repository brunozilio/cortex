//! The diff pane: the selected file through the `DiffView` component, with
//! the comments on it (published, drafts, the one being written) hung as
//! annotations, inside a scroll view.

use superfik_core::{CommentOrigin, CommentSide, CommentState, FileDiff, LineKind, ReviewComment};
use superfik_i18n::{Strings, Text};
use superfik_render::Rect;
use superfik_ui::components::{
    Annotation, AnnotationKnob, DiffFile, DiffHunk, DiffLayout as UiLayout, DiffRow, DiffSide, DiffView, LineRef,
    RowKind, ScrollView,
};
use superfik_ui::{Painter, TextEdit};

use crate::state::{DiffLayout, ReviewScreen, Session};
use crate::view::target::{HitMap, ReviewTarget, Target};

pub fn to_props(file: &FileDiff) -> DiffFile {
    DiffFile {
        path: file.path().to_owned(),
        additions: file.additions(),
        deletions: file.deletions(),
        hunks: file
            .hunks
            .iter()
            .map(|h| DiffHunk {
                old_start: h.old_start,
                new_start: h.new_start,
                heading: h.heading.clone(),
                rows: h
                    .lines
                    .iter()
                    .map(|l| DiffRow {
                        kind: match l.kind {
                            LineKind::Context => RowKind::Context,
                            LineKind::Added => RowKind::Added,
                            LineKind::Removed => RowKind::Removed,
                        },
                        old_no: l.old_no,
                        new_no: l.new_no,
                        text: l.text.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn side(side: CommentSide) -> DiffSide {
    match side {
        CommentSide::Old => DiffSide::Old,
        CommentSide::New => DiffSide::New,
    }
}

fn core_side(side: DiffSide) -> CommentSide {
    match side {
        DiffSide::Old => CommentSide::Old,
        DiffSide::New => CommentSide::New,
    }
}

struct Shown<'a> {
    id: u64,
    comment: Option<&'a ReviewComment>,
}

#[allow(clippy::too_many_arguments, reason = "a view entry point: everything it paints from, nothing stored")]
pub fn paint(
    p: &mut Painter,
    _session: &Session,
    review: &ReviewScreen,
    file: &FileDiff,
    strings: &Strings,
    pane: Rect,
    hovered: Option<Target>,
    caret_visible: bool,
    hits: &mut HitMap,
) {
    let props = to_props(file);
    let labels = (
        strings.t(Text::ReviewPending),
        strings.t(Text::ReviewResolved),
        strings.t(Text::ReviewAgentBadge),
        strings.t(Text::ReviewSave),
        strings.t(Text::Cancel),
    );
    let hovered_knob = |id: u64| match hovered {
        Some(Target::Review(ReviewTarget::Knob { comment, knob })) if comment == id => Some(knob),
        _ => None,
    };
    let empty = TextEdit::default();
    let mut shown: Vec<Shown> = Vec::new();
    let mut annotations: Vec<Annotation> = Vec::new();
    let path = file.path();
    for comment in review.comments_on(path) {
        let editing = review.editor.as_ref().filter(|e| e.comment == Some(comment.id)).map(|e| &e.body);
        let badge = match (comment.state, comment.origin) {
            (CommentState::Resolved, _) => Some(labels.1.as_str()),
            (CommentState::Draft, CommentOrigin::Agent) => Some(labels.2.as_str()),
            (CommentState::Draft, CommentOrigin::User) => Some(labels.0.as_str()),
            (CommentState::Published, _) => None,
        };
        shown.push(Shown { id: comment.id, comment: Some(comment) });
        annotations.push(Annotation {
            at: LineRef { side: side(comment.side), line: comment.line },
            author: &comment.author,
            badge,
            body: &comment.body,
            draft: comment.is_draft(),
            resolved: comment.state == CommentState::Resolved,
            editing,
            hovered_knob: hovered_knob(comment.id),
            caret_visible,
            save_label: &labels.3,
            cancel_label: &labels.4,
        });
    }
    if let Some(editor) = review.editor.as_ref().filter(|e| e.comment.is_none() && e.path == path) {
        shown.push(Shown { id: 0, comment: None });
        annotations.push(Annotation {
            at: LineRef { side: side(editor.side), line: editor.line },
            author: "",
            badge: Some(labels.0.as_str()),
            body: "",
            draft: true,
            resolved: false,
            editing: Some(&editor.body),
            hovered_knob: hovered_knob(0),
            caret_visible,
            save_label: &labels.3,
            cancel_label: &labels.4,
        });
    }
    let _ = &empty;
    let hovered_line = match hovered {
        Some(Target::Review(ReviewTarget::Line { side: s, line })) => Some(LineRef { side: side(s), line }),
        _ => None,
    };
    let layout = match review.layout {
        DiffLayout::Stacked => UiLayout::Stacked,
        DiffLayout::Split => UiLayout::Split,
    };
    let view = DiffView {
        file: &props,
        layout,
        selection: review.selection.map(|s| (side(s.side), s.from, s.to)),
        hovered: hovered_line,
        annotations: &annotations,
        scroll_x: 0.0,
    };
    let width = pane.w - ScrollView::THUMB_WIDTH - 8.0;
    let (content_height, overflow_x) = view.measure(p, width);
    let max_x = overflow_x.max(0.0);
    let view = DiffView { scroll_x: review.diff_scroll_x.min(max_x), ..view };
    let scroll = ScrollView { content_height, offset: review.diff_scroll };
    let (diff_hits, _) =
        scroll.paint(p, pane, |p, origin| view.paint(p, Rect::new(origin.x, origin.y, width, content_height)));
    for (rect, at) in diff_hits.lines {
        let rect = rect.intersect(&pane);
        hits.add_screen(rect, Target::Review(ReviewTarget::Line { side: core_side(at.side), line: at.line }));
    }
    for (rect, index, knob) in diff_hits.knobs {
        let Some(entry) = shown.get(index) else { continue };
        let knob = match (knob, entry.comment) {
            (AnnotationKnob::Resolve | AnnotationKnob::Reopen, Some(c)) if c.thread.is_none() => continue,
            _ => knob,
        };
        hits.add_screen(rect.intersect(&pane), Target::Review(ReviewTarget::Knob { comment: entry.id, knob }));
    }
}
