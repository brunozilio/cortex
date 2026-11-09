//! A sheet is a large popover: it grows out of the surface that spawned it
//! (the goo stem, pill and panel of [`Popover`]), wears a node header, and
//! only then resolves its content from blur. Used for confirmations (open
//! a pull request, publish a review).

use superfik_render::{GooShape, Icon, Point, Quad, Rect};

use super::knob::Knob;
use super::popover::{Popover, Side};
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct Sheet<'a> {
    pub anchor: Point,
    pub side: Side,
    pub panel: Rect,
    pub source: Option<GooShape>,
    pub progress: f32,
    pub icon: Icon,
    pub tint: Option<superfik_render::Color>,
    pub title: &'a str,
    pub close_hovered: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SheetHits {
    pub panel: Rect,
    pub body: Rect,
    pub close: Rect,
    pub content_opacity: f32,
}

impl Sheet<'_> {
    pub fn paint(&self, p: &mut Painter, f: impl FnOnce(&mut Painter, Rect)) -> SheetHits {
        let frame = Popover {
            anchor: self.anchor,
            side: self.side,
            panel: self.panel,
            progress: self.progress,
            source: self.source,
            closing: false,
        }
        .paint(p);
        let panel = self.panel;
        let body = Rect::new(
            panel.x + metrics::NODE_PADDING,
            panel.y + metrics::NODE_HEADER,
            panel.w - metrics::NODE_PADDING * 2.0,
            panel.h - metrics::NODE_HEADER - metrics::NODE_PADDING,
        );
        let mut close = Rect::default();
        if frame.content_opacity > 0.0 {
            p.faded(frame.content_opacity, |p| {
                p.blurred(frame.content_blur, |p| {
                    let cy = panel.y + 21.0;
                    p.icon_at(self.icon, Point::new(panel.x + 22.5, cy), metrics::HEADER_ICON, self.tint);
                    p.label(self.title, typography::HEADER, panel.x + 35.0, cy + 1.0, p.palette().text_secondary);
                    close =
                        Knob::close(Point::new(panel.right() - 13.0 - Knob::RADIUS, cy), self.close_hovered).paint(p);
                    f(p, body);
                });
            });
        }
        SheetHits { panel, body, close, content_opacity: frame.content_opacity }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Button<'a> {
    pub label: &'a str,
    pub primary: bool,
    pub hovered: bool,
    pub enabled: bool,
}

impl Button<'_> {
    pub fn width(&self, p: &mut Painter) -> f32 {
        p.layout(self.label, typography::CHIP, None).width + 26.0
    }

    pub fn focus_ring(p: &mut Painter, rect: Rect) {
        let ring = rect.inset(-metrics::FOCUS_RING);
        p.quad(Quad::new(ring, metrics::CHIP_RADIUS + metrics::FOCUS_RING, p.palette().form_accent));
    }

    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Rect {
        let c = p.palette();
        let fill = match (self.primary, self.enabled, self.hovered) {
            (true, false, _) => c.send_disabled,
            (true, true, true) => c.text_hover,
            (true, true, false) => c.send,
            (false, _, true) => c.hover,
            (false, _, false) => c.well,
        };
        p.quad(Quad::new(rect, metrics::CHIP_RADIUS, fill));
        let text = if self.primary {
            c.send_glyph
        } else if self.enabled {
            c.text_body
        } else {
            c.text_secondary
        };
        let layout = p.layout(self.label, typography::CHIP, None);
        p.label(self.label, typography::CHIP, rect.x + (rect.w - layout.width) * 0.5, rect.y + rect.h * 0.5, text);
        rect
    }
}
