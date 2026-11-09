//! A carved text field: placeholder or typed text, selection and a caret.
//! Single-line fields keep one line; multi-line ones wrap and grow with the
//! rect they are given.

use superfik_render::{Point, Quad, Rect, TextLayout};

use crate::painter::Painter;
use crate::text_edit::TextEdit;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextField<'a> {
    pub edit: &'a TextEdit,
    pub placeholder: &'a str,
    pub focused: bool,
    pub caret_visible: bool,
    pub multiline: bool,
    pub secret: bool,
}

const MASK: &str = "•";

pub const INSET: f32 = 10.0;

impl TextField<'_> {
    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Rect {
        p.quad(Quad::new(rect, metrics::WELL_RADIUS, p.palette().well));
        let style = typography::INPUT;
        let area = text_area(rect, style.line_height, self.multiline);
        let width = self.multiline.then_some(area.w);
        p.clipped(rect.inset(1.0), |p| {
            if self.edit.text().is_empty() {
                let layout = p.layout(self.placeholder, style, width);
                p.text(&layout, Point::new(area.x, area.y), p.palette().placeholder);
                if self.focused && self.caret_visible {
                    caret(p, Point::new(area.x, area.y), style.line_height);
                }
                return;
            }
            let text = self.edit.text();
            let (shown, caret_at) = if self.secret {
                let before = text[..self.edit.caret()].chars().count();
                (MASK.repeat(text.chars().count()), before * MASK.len())
            } else {
                (text.to_owned(), self.edit.caret())
            };
            let layout = p.layout(&shown, style, width);
            if let Some((a, b)) = self.edit.selection().filter(|_| !self.secret) {
                selection(p, &layout, Point::new(area.x, area.y), a, b, style.line_height);
            }
            p.text(&layout, Point::new(area.x, area.y), p.palette().text);
            if self.focused && self.caret_visible {
                let (x, top) = caret_position(&layout, caret_at);
                caret(p, Point::new(area.x + x, area.y + top), style.line_height);
            }
        });
        rect
    }
}

pub fn text_area(rect: Rect, line_height: f32, multiline: bool) -> Rect {
    let top = if multiline { 7.0 } else { ((rect.h - line_height) / 2.0).max(0.0) };
    Rect::new(rect.x + INSET, rect.y + top, rect.w - INSET * 2.0, rect.h - top * 2.0)
}

pub fn selection(p: &mut Painter, layout: &TextLayout, origin: Point, a: usize, b: usize, line_height: f32) {
    for line in &layout.lines {
        let glyphs: Vec<_> = line.glyphs.iter().filter(|g| g.start >= a && g.end <= b).collect();
        if let (Some(first), Some(last)) = (glyphs.first(), glyphs.last()) {
            let rect = Rect::new(origin.x + first.x, origin.y + line.top, last.x + last.w - first.x, line_height);
            p.quad(Quad::new(rect, 2.0, p.palette().agent.faded(0.28)));
        }
    }
}

pub fn caret_position(layout: &TextLayout, offset: usize) -> (f32, f32) {
    let mut position = (0.0, 0.0);
    for line in &layout.lines {
        let after_newline = layout.text.get(..line.start).is_some_and(|t| t.ends_with(['\n', '\r']));
        if after_newline && line.start <= offset {
            position = (line.glyphs.first().map_or(line.width, |g| g.x), line.top);
        }
        for glyph in &line.glyphs {
            if glyph.start < offset {
                position = (glyph.x + glyph.w, line.top);
            }
        }
        if offset == 0 {
            return (0.0, line.top);
        }
    }
    position
}

pub fn caret(p: &mut Painter, at: Point, height: f32) {
    p.quad(Quad::new(Rect::new(at.x, at.y + 2.0, 1.5, height - 4.0), 0.75, p.palette().text));
}

#[cfg(test)]
mod tests {
    use superfik_render::TextSystem;

    use super::*;

    #[test]
    fn a_single_line_sits_in_the_middle_of_the_well() {
        let well = Rect::new(0.0, 100.0, 300.0, 30.0);
        let area = text_area(well, 22.0, false);
        assert_eq!(area.y - well.y, (well.y + well.h) - (area.y + 22.0), "equal space above and below");
        assert_eq!(text_area(well, 22.0, true).y, 107.0, "multi-line text keeps its top inset");
    }

    #[test]
    fn the_caret_moves_to_the_line_a_newline_opens() {
        let mut text = TextSystem::new();
        let layout = text.layout("ab\ncd\n", typography::INPUT, Some(400.0));
        let line = typography::INPUT.line_height;
        assert_eq!(caret_position(&layout, 3), (0.0, line), "start of the second line");
        assert_eq!(caret_position(&layout, 6), (0.0, line * 2.0), "the empty line after a trailing newline");
        let (x, top) = caret_position(&layout, 4);
        assert!(x > 0.0 && top == line, "after the c: {x}, {top}");
    }
}

#[cfg(test)]
mod inset_tests {
    use superfik_render::{Flow, TextSystem};

    use super::*;

    #[test]
    fn the_caret_on_an_empty_narrowed_line_clears_the_inset() {
        let mut text = TextSystem::new();
        let flow = Flow { width: 400.0, inset_after: 2, inset: 60.0 };
        let layout = text.layout_flow("a\nb\n", typography::INPUT, flow);
        let (x, top) = caret_position(&layout, 4);
        assert_eq!(top, typography::INPUT.line_height * 2.0);
        assert_eq!(x, 60.0, "after the inset, not over the mic");
    }
}
