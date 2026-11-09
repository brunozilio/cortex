//! `‹ 2 / 6 ›` under a paged node's rows: arrow knobs either side of the
//! page counter. The page lives with the caller; this only paints.

use superfik_render::{Point, Rect};

use super::knob::Knob;
use crate::icons;
use crate::painter::Painter;
use crate::theme::typography;

const DISABLED: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
pub struct Pager {
    pub page: usize,
    pub pages: usize,
    pub hovered_prev: bool,
    pub hovered_next: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PagerHits {
    pub prev: Option<Rect>,
    pub next: Option<Rect>,
}

impl Pager {
    pub fn paint(&self, p: &mut Painter, rect: Rect) -> PagerHits {
        let cy = rect.y + rect.h * 0.5;
        let inset = Knob::RADIUS + 4.0;
        let has_prev = self.page > 0;
        let has_next = self.page + 1 < self.pages;
        let arrow = |p: &mut Painter, x: f32, glyph, hovered: bool, enabled: bool| {
            let knob = Knob { center: Point::new(x, cy), hovered: hovered && enabled, glyph };
            let rect = p.faded(if enabled { 1.0 } else { DISABLED }, |p| knob.paint(p));
            enabled.then_some(rect)
        };
        let prev = arrow(p, rect.x + inset, icons::CHEVRON_LEFT, self.hovered_prev, has_prev);
        let next = arrow(p, rect.right() - inset, icons::CHEVRON_RIGHT, self.hovered_next, has_next);
        let counter = format!("{} / {}", self.page + 1, self.pages);
        let width = p.layout(&counter, typography::CAPTION, None).width;
        p.label(&counter, typography::CAPTION, rect.center().x - width * 0.5, cy, p.palette().text_subtle);
        PagerHits { prev, next }
    }
}
