//! The send / stop button that nests in the composer's corner notch.

use superfik_render::{Quad, Rect};

use crate::icons;
use crate::painter::Painter;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendState {
    Disabled,
    Ready,
    Running,
}

#[derive(Debug, Clone, Copy)]
pub struct SendButton {
    pub state: SendState,
    pub hovered: bool,
}

impl SendButton {
    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Rect {
        let (bg, glyph, icon) = match self.state {
            SendState::Disabled => (p.palette().send_disabled, p.palette().icon_muted, icons::ARROW_UP),
            SendState::Ready => (p.palette().send, p.palette().send_glyph, icons::ARROW_UP),
            SendState::Running => (p.palette().send, p.palette().send_glyph, icons::STOP),
        };
        let bg = if self.hovered && self.state != SendState::Disabled { p.palette().text } else { bg };
        p.quad(Quad::new(rect, metrics::CHIP_RADIUS, bg));
        let size = if icon == icons::STOP { metrics::SEND_GLYPH } else { metrics::ICON };
        p.icon_at(icon, rect.center(), size, Some(glyph));
        rect
    }
}
