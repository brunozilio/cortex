//! Notes node: the project notes linked to the session, as sheets of
//! coloured paper. A click on a note writes in it; a box ticks its item;
//! the knob on a hovered note takes it off the session. Below them, once
//! `/notes` shows them, the project's other notes (a click links one), and
//! a row that starts a new note.

use superfik_core::note::{Line, Note, NoteId, Paper};
use superfik_i18n::Text;
use superfik_render::Rect;
use superfik_ui::components::{NodeFrame, NoteLine, Row, StickyNote};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, TextEdit, icons};

use super::{NodeCtx, node_height, reveal};
use crate::view::target::{NoteTarget, Target};

const LOOSE: usize = 5;

#[derive(Debug, Clone, Copy)]
pub struct NotesProps<'a> {
    pub linked: &'a [&'a Note],
    pub loose: &'a [&'a Note],
    pub editing: Option<(Option<NoteId>, &'a TextEdit, Paper)>,
    pub caret_visible: bool,
}

fn shown(note: &Note) -> (Vec<NoteLine<'_>>, usize) {
    let lines: Vec<NoteLine> = note
        .lines()
        .into_iter()
        .skip_while(|line| matches!(line, Line::Text(t) if t.trim().is_empty()))
        .skip(1)
        .map(|line| match line {
            Line::Text(text) => NoteLine::Text(text),
            Line::Check { done, text } => NoteLine::Check { done, text },
        })
        .collect();
    let more = lines.len().saturating_sub(metrics::NOTE_MAX_LINES);
    (lines.into_iter().take(metrics::NOTE_MAX_LINES).collect(), more)
}

fn paper_index(paper: Paper) -> usize {
    Paper::ALL.iter().position(|p| *p == paper).unwrap_or(0)
}

fn edit_lines(edit: &TextEdit) -> usize {
    edit.text().split('\n').count()
}

fn card_heights(props: &NotesProps) -> Vec<f32> {
    let mut heights = Vec::new();
    if let Some((None, edit, _)) = props.editing {
        heights.push(StickyNote::height(0, false, Some(edit_lines(edit))));
    }
    for note in props.linked {
        let height = match props.editing {
            Some((Some(id), edit, _)) if id == note.id => StickyNote::height(0, false, Some(edit_lines(edit))),
            _ => {
                let (lines, more) = shown(note);
                StickyNote::height(lines.len(), more > 0, None)
            }
        };
        heights.push(height);
    }
    heights
}

fn rows(props: &NotesProps) -> usize {
    props.loose.len().min(LOOSE) + 1
}

pub fn height(props: &NotesProps) -> f32 {
    let cards = card_heights(props);
    let cards_height: f32 = cards.iter().map(|h| h + metrics::NOTE_GAP).sum();
    let rows = rows(props) as f32;
    node_height(cards_height + rows * metrics::ROW_HEIGHT + (rows - 1.0) * metrics::ROW_GAP)
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, props: &NotesProps) {
    let title = ctx.strings.t(Text::Notes);
    let count = props.linked.len().to_string();
    let mut frame = NodeFrame::new(ctx.rect, icons::NOTE, Some(p.palette().notes), &title);
    frame.count = (!props.linked.is_empty()).then_some(count.as_str());
    frame.busy = ctx.busy;
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.paint(p);
    let body = ctx.body();
    reveal(p, ctx.content, |p| paint_body(p, ctx, props, body));
}

fn paint_body(p: &mut Painter, ctx: &mut NodeCtx, props: &NotesProps, body: Rect) {
    let heights = card_heights(props);
    let mut heights = heights.into_iter();
    let mut y = body.y;
    let placeholder = ctx.strings.t(Text::NotePlaceholder);
    if let Some((None, edit, paper)) = props.editing {
        let h = heights.next().unwrap_or_default();
        let rect = Rect::new(body.x, y, body.w, h);
        paint_draft(p, ctx, rect, (edit, paper), &placeholder, props.caret_visible);
        y += h + metrics::NOTE_GAP;
    }
    for note in props.linked {
        let h = heights.next().unwrap_or_default();
        let rect = Rect::new(body.x, y, body.w, h);
        match props.editing {
            Some((Some(id), edit, paper)) if id == note.id => {
                paint_draft(p, ctx, rect, (edit, paper), &placeholder, props.caret_visible);
            }
            _ => paint_card(p, ctx, note, rect),
        }
        y += h + metrics::NOTE_GAP;
    }
    let row = |y: f32| Rect::new(body.x, y, body.w, metrics::ROW_HEIGHT);
    for note in props.loose.iter().take(LOOSE) {
        let target = Target::Note(NoteTarget::Link(note.id));
        Row::new(note.title())
            .icon(icons::PLUS_CIRCLE, Some(p.palette().icon_muted))
            .muted(true)
            .hovered(ctx.is_hovered(target))
            .paint(p, row(y));
        ctx.hits.add(row(y), target);
        y += metrics::ROW_HEIGHT + metrics::ROW_GAP;
    }
    let new = Target::Note(NoteTarget::New);
    let label = ctx.strings.t(Text::NewNote);
    Row::new(&label).icon(icons::PLUS, Some(p.palette().notes)).hovered(ctx.is_hovered(new)).paint(p, row(y));
    ctx.hits.add(row(y), new);
}

fn paint_card(p: &mut Painter, ctx: &mut NodeCtx, note: &Note, rect: Rect) {
    let (lines, more) = shown(note);
    let more_label = ctx.strings.t(Text::NoteMore(more));
    let card = Target::Note(NoteTarget::Card(note.id));
    let knob = Target::Note(NoteTarget::Unlink(note.id));
    let hovered = matches!(ctx.hovered, Some(t) if t == card || t == knob
        || matches!(t, Target::Note(NoteTarget::Check(id, _)) if id == note.id));
    let hits = StickyNote {
        paper: paper_index(note.paper),
        title: note.title(),
        lines: &lines,
        more,
        more_label: &more_label,
        hovered,
        knob_hovered: ctx.is_hovered(knob),
        editing: None,
        placeholder: "",
        caret_visible: false,
    }
    .paint(p, rect);
    ctx.hits.add(hits.card, card);
    for (item, rect) in hits.checks {
        ctx.hits.add(rect, Target::Note(NoteTarget::Check(note.id, item)));
    }
    if let Some(rect) = hits.knob {
        ctx.hits.add(rect, knob);
    }
}

fn paint_draft(
    p: &mut Painter,
    ctx: &mut NodeCtx,
    rect: Rect,
    (edit, paper): (&TextEdit, Paper),
    placeholder: &str,
    caret: bool,
) {
    let hits = StickyNote {
        paper: paper_index(paper),
        title: "",
        lines: &[],
        more: 0,
        more_label: "",
        hovered: false,
        knob_hovered: false,
        editing: Some(edit),
        placeholder,
        caret_visible: caret,
    }
    .paint(p, rect);
    ctx.hits.add(hits.card, Target::Note(NoteTarget::Draft));
    for (i, rect) in hits.swatches {
        ctx.hits.add(rect, Target::Note(NoteTarget::Paper(i)));
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::note::Author;

    use super::*;

    fn note(body: &str) -> Note {
        Note { id: NoteId(0), body: body.into(), paper: Paper::Blue, author: Author::User, updated: 0, revision: 0 }
    }

    #[test]
    fn a_card_shows_the_lines_under_its_title_and_counts_the_rest() {
        let n = note("\nTrip\n- [ ] passport\n- [x] tickets");
        let (lines, more) = shown(&n);
        assert_eq!(
            lines,
            vec![NoteLine::Check { done: false, text: "passport" }, NoteLine::Check { done: true, text: "tickets" }]
        );
        assert_eq!(more, 0);
        let long = note(&format!("Title\n{}", ["x"; metrics::NOTE_MAX_LINES + 2].join("\n")));
        assert_eq!(shown(&long).1, 2);
    }

    #[test]
    fn the_node_grows_with_its_notes_and_rows() {
        let a = note("A\nline");
        let empty = NotesProps { linked: &[], loose: &[], editing: None, caret_visible: false };
        let one = NotesProps { linked: &[&a], ..empty };
        let loose = NotesProps { loose: &[&a], ..empty };
        assert!(height(&one) > height(&empty));
        assert_eq!(height(&loose) - height(&empty), metrics::ROW_HEIGHT + metrics::ROW_GAP);
    }
}
