//! A note as the Notes node shows it: a sheet of coloured paper with a
//! band across its top, its title, and its first lines, checklist items
//! with a box each. While it is edited it shows its whole text instead,
//! with a caret. Stateless: the paper, lines and edit come in as props;
//! the card, each box and the remove knob come back as hits.

use superfik_render::{Point, Quad, Rect};

use super::knob::Knob;
use super::text_field::{caret, caret_position, selection};
use crate::icons;
use crate::painter::Painter;
use crate::text_edit::TextEdit;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteLine<'a> {
    Text(&'a str),
    Check { done: bool, text: &'a str },
}

#[derive(Debug, Clone, Copy)]
pub struct StickyNote<'a> {
    pub paper: usize,
    pub title: &'a str,
    pub lines: &'a [NoteLine<'a>],
    pub more: usize,
    pub more_label: &'a str,
    pub hovered: bool,
    pub knob_hovered: bool,
    pub editing: Option<&'a TextEdit>,
    pub placeholder: &'a str,
    pub caret_visible: bool,
}

#[derive(Debug, Clone, Default)]
pub struct StickyNoteHits {
    pub card: Rect,
    pub checks: Vec<(usize, Rect)>,
    pub knob: Option<Rect>,
    pub swatches: Vec<(usize, Rect)>,
}

impl StickyNote<'_> {
    pub fn height(lines: usize, more: bool, edit_lines: Option<usize>) -> f32 {
        let line = typography::NOTE_LINE.line_height;
        let body = match edit_lines {
            Some(n) => n.max(metrics::NOTE_MIN_EDIT_LINES) as f32 * line + metrics::NOTE_PADDING + metrics::NOTE_SWATCH,
            None => typography::NOTE_TITLE.line_height + (lines + usize::from(more)) as f32 * line,
        };
        metrics::NOTE_BAND + metrics::NOTE_PADDING * 2.0 + body
    }

    pub fn paint(&self, p: &mut Painter, rect: Rect) -> StickyNoteHits {
        let tones = p.palette().paper;
        let paper = self.paper.min(tones.sheets.len() - 1);
        p.quad(Quad::new(rect, metrics::NOTE_RADIUS, tones.bands[paper]));
        let sheet = Rect::new(rect.x, rect.y + metrics::NOTE_BAND, rect.w, rect.h - metrics::NOTE_BAND);
        p.quad(Quad::new(sheet, metrics::NOTE_RADIUS, tones.sheets[paper]));
        let pad = metrics::NOTE_PADDING;
        let inner = Rect::new(sheet.x + pad, sheet.y + pad, sheet.w - pad * 2.0, sheet.h - pad * 2.0);
        let mut hits = StickyNoteHits { card: rect, ..StickyNoteHits::default() };
        p.clipped(sheet, |p| match self.editing {
            Some(edit) => {
                let foot = metrics::NOTE_PADDING + metrics::NOTE_SWATCH;
                self.paint_edit(p, Rect::new(inner.x, inner.y, inner.w, inner.h - foot), edit);
                hits.swatches = self.paint_swatches(p, inner);
            }
            None => self.paint_lines(p, inner, &mut hits),
        });
        if self.hovered && self.editing.is_none() {
            let center = Point::new(rect.x + rect.w - Knob::RADIUS - 4.0, rect.y + Knob::RADIUS + 3.0);
            hits.knob = Some(Knob::close(center, self.knob_hovered).paint(p));
        }
        hits
    }

    fn paint_lines(&self, p: &mut Painter, inner: Rect, hits: &mut StickyNoteHits) {
        let tones = p.palette().paper;
        let title = typography::NOTE_TITLE;
        let line = typography::NOTE_LINE;
        p.label_fit(self.title, title, inner.x, inner.y + title.line_height / 2.0, inner.w, tones.ink);
        let mut y = inner.y + title.line_height;
        let mut item = 0;
        for entry in self.lines {
            let center = y + line.line_height / 2.0;
            match *entry {
                NoteLine::Text(text) => p.label_fit(text, line, inner.x, center, inner.w, tones.ink),
                NoteLine::Check { done, text } => {
                    let size = metrics::NOTE_CHECK;
                    let mark = Rect::new(inner.x, center - size / 2.0, size, size);
                    check_box(p, mark, done, tones.sheets[self.paper.min(tones.sheets.len() - 1)]);
                    let gap = 6.0;
                    let color = if done { tones.ink_done } else { tones.ink };
                    p.label_fit(text, line, inner.x + size + gap, center, inner.w - size - gap, color);
                    hits.checks.push((item, mark.inset(-4.0)));
                    item += 1;
                }
            }
            y += line.line_height;
        }
        if self.more > 0 {
            p.label_fit(self.more_label, line, inner.x, y + line.line_height / 2.0, inner.w, tones.ink_done);
        }
    }

    fn paint_swatches(&self, p: &mut Painter, inner: Rect) -> Vec<(usize, Rect)> {
        let tones = p.palette().paper;
        let size = metrics::NOTE_SWATCH;
        let y = inner.y + inner.h - size;
        (0..tones.sheets.len())
            .map(|i| {
                let rect = Rect::new(inner.x + i as f32 * (size + metrics::NOTE_SWATCH_GAP), y, size, size);
                let edge = if i == self.paper { 2.0 } else { 1.0 };
                let ring = if i == self.paper { tones.ink } else { tones.ink.faded(0.35) };
                p.quad(Quad::new(rect, size / 2.0, ring));
                p.quad(Quad::new(rect.inset(edge), size / 2.0 - edge, tones.sheets[i]));
                (i, rect.inset(-3.0))
            })
            .collect()
    }

    fn paint_edit(&self, p: &mut Painter, inner: Rect, edit: &TextEdit) {
        let tones = p.palette().paper;
        let style = typography::NOTE_LINE;
        let origin = Point::new(inner.x, inner.y);
        if edit.text().is_empty() && !self.placeholder.is_empty() {
            let hint = p.layout(self.placeholder, style, Some(inner.w));
            p.text(&hint, origin, tones.ink_done);
        }
        let layout = p.layout(edit.text(), style, Some(inner.w));
        if let Some((a, b)) = edit.selection() {
            selection(p, &layout, origin, a, b, style.line_height);
        }
        p.text(&layout, origin, tones.ink);
        if self.caret_visible {
            let (x, top) = caret_position(&layout, edit.caret());
            caret(p, Point::new(origin.x + x, origin.y + top), style.line_height);
        }
    }
}

fn check_box(p: &mut Painter, rect: Rect, done: bool, sheet: superfik_render::Color) {
    let tones = p.palette().paper;
    let radius = 3.5;
    if done {
        p.quad(Quad::new(rect, radius, tones.ink_done));
        p.icon(icons::CHECK, rect.inset(1.0), Some(sheet));
    } else {
        let edge = 1.25;
        p.quad(Quad::new(rect, radius, tones.ink.faded(0.55)));
        p.quad(Quad::new(rect.inset(edge), radius - edge, sheet));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_grow_with_lines_and_editing_has_a_floor() {
        let one = StickyNote::height(1, false, None);
        assert_eq!(StickyNote::height(2, false, None) - one, typography::NOTE_LINE.line_height);
        assert_eq!(StickyNote::height(1, true, None), StickyNote::height(2, false, None));
        assert_eq!(StickyNote::height(0, false, Some(1)), StickyNote::height(0, false, Some(3)));
        assert!(StickyNote::height(0, false, Some(5)) > StickyNote::height(0, false, Some(3)));
    }
}
