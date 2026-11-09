//! Composer text: placeholder, typed text, selection and a blinking caret.

use std::sync::Arc;

use superfik_render::{Color, Flow, Point, Rect, TextLayout};

use super::ComposerProps;
use super::highlight::{covering, display_text, paint_marks, paint_pills, spacing};
use crate::components::text_field::{caret, caret_position, selection};
use crate::markdown::{self, Kind};
use crate::painter::Painter;
use crate::theme::colors::Palette;
use crate::theme::typography;

#[derive(Debug, Clone, Default)]
pub(super) struct Scrolled {
    pub scroll: f32,
    pub overflow: f32,
    pub text: Option<(Arc<TextLayout>, Point)>,
}

pub(super) fn paint(p: &mut Painter, props: &ComposerProps, area: Rect, flow: Flow) -> Scrolled {
    let edit = props.edit;
    if edit.text().is_empty() {
        let origin = Point::new(area.x, area.y);
        let layout = p.layout(props.placeholder, typography::INPUT, Some(area.w));
        p.text(&layout, origin, p.palette().placeholder);
        if props.caret_visible {
            caret(p, origin, layout.height.min(typography::INPUT.line_height));
        }
        return Scrolled::default();
    }
    let shown = display_text(edit.text(), props.highlights);
    let marked = markdown::scan(edit.text());
    let layout = p.layout_styled(
        &shown,
        typography::INPUT,
        flow,
        &spacing(edit.text(), props.highlights),
        &markdown::runs(&marked),
    );
    let line_height = typography::INPUT.line_height;
    let (caret_x, caret_top) = caret_position(&layout, edit.caret());
    let overflow = (layout.height - area.h).max(0.0);
    let scroll = scroll_to(props.scroll, overflow, props.follow_caret.then_some((caret_top, line_height)), area.h);
    let origin = Point::new(area.x, area.y - scroll);
    let pad = crate::theme::metrics::TOKEN_PAD;
    p.clipped(Rect::new(area.x - pad, area.y, area.w + pad * 2.0, area.h), |p| {
        if let Some((a, b)) = edit.selection() {
            selection(p, &layout, origin, a, b, line_height);
        }
        paint_pills(p, &layout, origin, line_height, props.highlights);
        let c = p.palette();
        let text = c.text;
        p.text_with(&layout, origin, |at| match covering(props.highlights, at) {
            Some(h) if at == h.start => (text.with_alpha(0.0), 0.0),
            Some(h) => (h.color.mix(text, 0.35), 0.0),
            None => (markdown_color(&marked, at, text, c), 0.0),
        });
        paint_marks(p, &layout, origin, props.highlights);
        if props.caret_visible {
            caret(p, Point::new(origin.x + caret_x, origin.y + caret_top), line_height);
        }
        if let Some(rest) = props.completion {
            let ghost = p.layout(rest, typography::INPUT, None);
            p.text(&ghost, Point::new(origin.x + caret_x, origin.y + caret_top), c.text_tertiary);
        }
    });
    Scrolled { scroll, overflow, text: Some((layout, origin)) }
}

fn markdown_color(marked: &[markdown::Span], at: usize, text: Color, c: &Palette) -> Color {
    let Some(span) = marked.iter().rev().find(|s| s.whole().contains(&at)) else { return text };
    if !span.inner.contains(&at) {
        return c.text_tertiary;
    }
    match span.kind {
        Kind::Code => c.text_code,
        Kind::Link => c.form_accent,
        Kind::Bold | Kind::Italic => text,
    }
}

fn scroll_to(scroll: f32, overflow: f32, caret: Option<(f32, f32)>, height: f32) -> f32 {
    let scroll = scroll.clamp(0.0, overflow);
    match caret {
        Some((top, line_height)) => scroll.clamp((top + line_height - height).max(0.0), top.max(0.0)),
        None => scroll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_scrolls_within_the_text() {
        assert_eq!(scroll_to(30.0, 100.0, None, 88.0), 30.0);
        assert_eq!(scroll_to(-5.0, 100.0, None, 88.0), 0.0);
        assert_eq!(scroll_to(500.0, 100.0, None, 88.0), 100.0);
        assert_eq!(scroll_to(30.0, 0.0, None, 88.0), 0.0, "text that fits never scrolls");
    }

    #[test]
    fn editing_brings_the_caret_line_into_view_and_no_further() {
        assert_eq!(scroll_to(0.0, 100.0, Some((44.0, 22.0)), 88.0), 0.0, "third line already visible");
        assert_eq!(scroll_to(0.0, 100.0, Some((88.0, 22.0)), 88.0), 22.0, "fifth line scrolls one line");
        assert_eq!(scroll_to(60.0, 100.0, Some((0.0, 22.0)), 88.0), 0.0, "caret back at the top");
        assert_eq!(scroll_to(40.0, 100.0, Some((66.0, 22.0)), 88.0), 40.0, "already in view: stays put");
    }
}
