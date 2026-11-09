//! Composer controls: square icon buttons, toggle chips and the stepper.

use superfik_render::{Color, Icon, Point, Quad, Rect};

use crate::painter::Painter;
use crate::theme::colors::Palette;
use crate::theme::{metrics, typography};

fn control_color(c: &Palette, active: bool, hovered: bool) -> Color {
    match (active, hovered) {
        (true, _) => c.control_active,
        (false, true) => c.hover,
        (false, false) => c.well,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SquareButton {
    pub icon: Icon,
    pub hovered: bool,
    pub lit: bool,
    pub dot: bool,
}

impl SquareButton {
    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Rect {
        let c = p.palette();
        let (fill, glyph) =
            if self.lit { (c.send, c.send_glyph) } else { (control_color(c, false, self.hovered), c.icon) };
        p.quad(Quad::new(rect, metrics::CHIP_RADIUS, fill));
        p.icon_at(self.icon, rect.center(), metrics::CONTROL_ICON, Some(glyph));
        if self.dot {
            let r = metrics::FOOTER_DOT_RADIUS;
            let (x, y) = (rect.right() - metrics::FOOTER_DOT_INSET, rect.y + metrics::FOOTER_DOT_INSET);
            p.quad(Quad::new(Rect::new(x - r, y - r, r * 2.0, r * 2.0), r, c.form_accent));
        }
        rect
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Chip<'a> {
    pub icon: Icon,
    pub label: &'a str,
    pub active: bool,
    pub hovered: bool,
    pub tint: Option<Color>,
}

impl Chip<'_> {
    pub fn width(&self, p: &mut Painter) -> f32 {
        p.layout(self.label, typography::CHIP, None).width + 38.0
    }

    pub fn paint(&self, p: &mut Painter, origin: Point) -> Rect {
        let rect = Rect::new(origin.x, origin.y, self.width(p), metrics::CHIP_HEIGHT);
        let fill = control_color(p.palette(), self.active, self.hovered);
        let fill = self.tint.map_or(fill, |tint| fill.mix(tint, metrics::CHIP_TINT));
        p.quad(Quad::new(rect, metrics::CHIP_RADIUS, fill));
        let cy = rect.y + rect.h * 0.5;
        let icon = self.tint.unwrap_or(p.palette().icon);
        p.icon_at(self.icon, Point::new(rect.x + 14.5, cy), metrics::CHIP_ICON, Some(icon));
        let color = match self.tint {
            Some(tint) => tint,
            None if self.active => p.palette().text_body,
            None => p.palette().text_secondary,
        };
        p.label(self.label, typography::CHIP, rect.x + 29.0, cy, color);
        rect
    }
}
