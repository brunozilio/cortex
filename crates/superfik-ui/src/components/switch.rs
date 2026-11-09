//! An on/off switch: a pill track, accented while on, with a round knob at
//! the end it is set to. Faded while it cannot be used.

use superfik_render::{Quad, Rect};

use crate::painter::Painter;
use crate::theme::colors::DISABLED_OPACITY;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switch {
    pub on: bool,
    pub enabled: bool,
}

impl Switch {
    pub const SIZE: (f32, f32) = (metrics::SWITCH_WIDTH, metrics::SWITCH_HEIGHT);

    pub fn paint(&self, p: &mut Painter, right: f32, center_y: f32) -> Rect {
        let (w, h) = Self::SIZE;
        let track = Rect::new(right - w, (center_y - h * 0.5).round(), w, h);
        let opacity = if self.enabled { 1.0 } else { DISABLED_OPACITY };
        p.faded(opacity, |p| {
            let c = p.palette();
            let quad = Quad::new(track, h * 0.5, if self.on { c.form_accent } else { c.form_box });
            p.quad(if self.on { quad } else { quad.border(c.form_box_edge, 1.0) });
            let d = h - metrics::SWITCH_INSET * 2.0;
            let x = if self.on { track.right() - metrics::SWITCH_INSET - d } else { track.x + metrics::SWITCH_INSET };
            let knob = Rect::new(x, track.y + metrics::SWITCH_INSET, d, d);
            p.quad(Quad::new(knob, d * 0.5, c.form_mark).border(c.form_box_edge, if self.on { 0.0 } else { 0.5 }));
        });
        track
    }
}
