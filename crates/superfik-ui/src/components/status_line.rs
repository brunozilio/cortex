//! "Worked for 3s · 2 steps ▾" — the line above every reply.

use superfik_render::{Point, Rect};

use crate::icons;
use crate::painter::Painter;
use crate::theme::typography;

#[derive(Debug, Clone, Copy)]
pub struct StatusLine<'a> {
    pub label: &'a str,
    pub detail: &'a str,
    pub live: bool,
    pub time: f32,
    pub open: bool,
}

impl StatusLine<'_> {
    pub const HEIGHT: f32 = 18.0;

    pub fn paint(&self, p: &mut Painter, origin: Point) -> Rect {
        let cy = origin.y + Self::HEIGHT * 0.5;
        let layout = p.layout(self.label, typography::STATUS, None);
        let y = (cy - layout.height * 0.5).round();
        if self.live {
            let width = self.label.len().max(1) as f32;
            let sweep = (self.time * 0.9).fract() * (width + 8.0) - 4.0;
            let c = p.palette();
            p.text_with(&layout, Point::new(origin.x, y), |offset| {
                let d = (offset as f32 - sweep).abs();
                let glow = (1.0 - d / 4.0).clamp(0.0, 1.0);
                (c.status_label.mix(c.text, glow), 0.0)
            });
        } else {
            p.text(&layout, Point::new(origin.x, y), p.palette().status_label);
        }
        let detail = p.layout(self.detail, typography::STATUS, None);
        let dx = origin.x + layout.width + 8.0;
        p.text(&detail, Point::new(dx, y), p.palette().status_detail);
        let caret = Point::new(dx + detail.width + 10.0, cy);
        let glyph = if self.open { icons::CARET_UP } else { icons::CARET_DOWN };
        p.icon_at(glyph, caret, 12.0, Some(p.palette().status_detail));
        Rect::new(origin.x, origin.y, caret.x + 6.0 - origin.x, Self::HEIGHT)
    }
}
