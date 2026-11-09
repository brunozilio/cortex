//! The goo popover: a drop grows out of its anchor, swells into a pill,
//! opens into a panel, and only then does the content resolve from blur.

use superfik_motion::{Easing, remap};
use superfik_render::{GooPrim, GooShape, Point, Rect};

use crate::painter::Painter;
use crate::theme::{metrics, motion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Up,
    Down,
    Left,
    Right,
}

impl Side {
    fn vector(self) -> Point {
        match self {
            Self::Up => Point::new(0.0, -1.0),
            Self::Down => Point::new(0.0, 1.0),
            Self::Left => Point::new(-1.0, 0.0),
            Self::Right => Point::new(1.0, 0.0),
        }
    }

    fn vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Popover {
    pub anchor: Point,
    pub side: Side,
    pub panel: Rect,
    pub progress: f32,
    pub source: Option<GooShape>,
    pub closing: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PopoverFrame {
    pub panel: Rect,
    pub content_opacity: f32,
    pub content_blur: f32,
}

const STEM_RADIUS: f32 = 5.0;
const PILL: (f32, f32) = (36.0, 14.0);

impl Popover {
    fn gap(&self) -> f32 {
        match self.side {
            Side::Up => self.anchor.y - self.panel.bottom(),
            Side::Down => self.panel.y - self.anchor.y,
            Side::Left => self.anchor.x - self.panel.right(),
            Side::Right => self.panel.x - self.anchor.x,
        }
        .max(0.0)
    }

    pub fn panel_at(&self, t: f32) -> Rect {
        let dir = self.side.vector();
        let gap = self.gap();
        let tip = Point::new(self.anchor.x + dir.x * gap, self.anchor.y + dir.y * gap);
        let (pw, ph) = if self.side.vertical() { PILL } else { (PILL.1, PILL.0) };
        let pill = Rect::new(tip.x - pw * 0.5 + dir.x * pw * 0.5, tip.y - ph * 0.5 + dir.y * ph * 0.5, pw, ph);
        let ease = if self.closing { Easing::OUT } else { Easing::CubicBezier(0.3, 1.25, 0.4, 1.0) };
        let (cross, along) = (ease.apply(remap(t, 0.22, 0.58)), ease.apply(remap(t, 0.34, 0.78)));
        let (tw, th) = if self.side.vertical() { (cross, along) } else { (along, cross) };
        let lerp = |a: f32, b: f32, k: f32| a + (b - a) * k;
        Rect::new(
            lerp(pill.x, self.panel.x, tw),
            lerp(pill.y, self.panel.y, th),
            lerp(pill.w, self.panel.w, tw),
            lerp(pill.h, self.panel.h, th),
        )
    }

    pub fn paint(&self, p: &mut Painter) -> PopoverFrame {
        let t = self.progress.clamp(0.0, 1.0);
        if let Some(source) = self.source {
            p.surface(GooPrim::new(source, p.palette().surface_top, metrics::GOO).ghost());
        }
        let dir = self.side.vector();
        let gap = self.gap();
        let stem_len = (gap + 6.0) * Easing::OUT.apply(remap(t, 0.0, 0.32));
        let start = Point::new(self.anchor.x - dir.x * 4.0, self.anchor.y - dir.y * 4.0);
        let end = Point::new(self.anchor.x + dir.x * stem_len, self.anchor.y + dir.y * stem_len);
        let neck = STEM_RADIUS * remap(t, 0.0, 0.12).max(0.2);
        p.surface(GooPrim::new(
            GooShape::Capsule { a: start, b: end, radius: neck },
            p.palette().surface_top,
            metrics::GOO,
        ));
        let mut panel = self.panel_at(t);
        if t > 0.0 {
            let k = Easing::OUT.apply(remap(t, 0.0, 0.2));
            let centre = panel.center();
            panel = Rect::new(centre.x - panel.w * k * 0.5, centre.y - panel.h * k * 0.5, panel.w * k, panel.h * k);
            let radius = metrics::NODE_RADIUS.min(panel.h * 0.5).min(panel.w * 0.5);
            p.surface(
                GooPrim::new(GooShape::RoundRect { rect: panel, radius }, p.palette().surface_top, metrics::GOO)
                    .gradient(p.palette().surface_top, p.palette().surface_bottom),
            );
        }
        let reveal = Easing::OUT.apply(remap(t, 0.62, 1.0));
        PopoverFrame { panel, content_opacity: reveal, content_blur: (1.0 - reveal) * motion::BLUR }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn popover() -> Popover {
        Popover {
            anchor: Point::new(100.0, 100.0),
            side: Side::Up,
            panel: Rect::new(40.0, 0.0, 200.0, 80.0),
            progress: 0.0,
            source: None,
            closing: false,
        }
    }

    #[test]
    fn fully_open_panel_matches_target() {
        assert_eq!(popover().panel_at(1.0), Rect::new(40.0, 0.0, 200.0, 80.0));
    }

    #[test]
    fn closed_panel_is_a_pill_at_the_stem_tip() {
        let pill = popover().panel_at(0.0);
        assert_eq!((pill.w, pill.h), PILL);
        assert_eq!(pill.center().x, 100.0);
        assert_eq!(pill.bottom(), 80.0);
    }

    #[test]
    fn closing_never_overshoots_the_panel() {
        let popover = Popover { closing: true, ..popover() };
        for step in 0..=100 {
            let panel = popover.panel_at(step as f32 / 100.0);
            assert!(panel.w <= 200.0 + 1e-3 && panel.h <= 80.0 + 1e-3);
        }
    }

    #[test]
    fn width_opens_before_height() {
        let mid = popover().panel_at(0.45);
        let w = (mid.w - PILL.0) / (200.0 - PILL.0);
        let h = (mid.h - PILL.1) / (80.0 - PILL.1);
        assert!(w > h);
    }
}
