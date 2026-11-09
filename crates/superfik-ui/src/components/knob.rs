//! The round close knob that hangs off removable nodes.

use superfik_render::{Icon, Point, Quad, Rect};

use crate::icons;
use crate::painter::{Painter, Plane};
use crate::theme::{colors, metrics};

#[derive(Debug, Clone, Copy)]
pub struct Knob {
    pub center: Point,
    pub hovered: bool,
    pub glyph: Icon,
}

impl Knob {
    pub const RADIUS: f32 = 6.5;

    pub fn close(center: Point, hovered: bool) -> Self {
        Self { center, hovered, glyph: icons::CLOSE }
    }

    pub fn paint(&self, p: &mut Painter) -> Rect {
        let r = Self::RADIUS + if self.hovered { 0.5 } else { 0.0 };
        let rect = Rect::new(self.center.x - r, self.center.y - r, r * 2.0, r * 2.0);
        let color = if self.hovered { p.palette().text } else { p.palette().knob };
        p.quad(Quad::new(rect, r, color));
        p.icon_at(self.glyph, self.center, 10.0, Some(p.palette().knob_glyph));
        rect.inset(-4.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RemoveBadge {
    pub center: Point,
    pub hovered: bool,
}

impl RemoveBadge {
    pub fn paint(&self, p: &mut Painter) -> Rect {
        let r = metrics::TILE_REMOVE_RADIUS + if self.hovered { 0.5 } else { 0.0 };
        let scrim = r + metrics::TILE_REMOVE_SCRIM;
        let square =
            |radius: f32| Rect::new(self.center.x - radius, self.center.y - radius, radius * 2.0, radius * 2.0);
        p.on_plane(Plane::Overlay, |p| {
            p.quad(Quad::new(square(scrim), scrim, p.palette().knob_scrim.faded(colors::KNOB_SCRIM_OPACITY)));
            let color = if self.hovered { p.palette().text } else { p.palette().knob };
            p.quad(Quad::new(square(r), r, color));
            p.icon_at(icons::CLOSE, self.center, metrics::TILE_REMOVE_GLYPH, Some(p.palette().knob_glyph));
        });
        square(scrim)
    }
}
