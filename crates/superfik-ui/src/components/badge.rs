//! Small count chip (e.g. the `2` next to "References").

use superfik_render::{Quad, Rect};

use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct Badge<'a> {
    pub text: &'a str,
}

impl Badge<'_> {
    pub fn width(&self, p: &mut Painter) -> f32 {
        (p.layout(self.text, typography::CAPTION, None).width + metrics::BADGE_PADDING * 2.0).max(metrics::BADGE_HEIGHT)
    }

    pub fn paint(&self, p: &mut Painter, right: f32, center_y: f32) -> Rect {
        let w = self.width(p);
        let layout = p.layout(self.text, typography::CAPTION, None);
        let rect = Rect::new(right - w, center_y - metrics::BADGE_HEIGHT * 0.5, w, metrics::BADGE_HEIGHT);
        p.quad(Quad::new(rect, metrics::BADGE_RADIUS, p.palette().badge));
        p.label(
            self.text,
            typography::CAPTION,
            rect.x + (w - layout.width) * 0.5,
            center_y,
            p.palette().text_secondary,
        );
        rect
    }
}
