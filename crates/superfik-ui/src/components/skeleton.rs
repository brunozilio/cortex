//! Placeholder row with a sweeping shimmer while content loads.

use superfik_render::{Fill, Pattern, Quad, Rect};

use crate::painter::Painter;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy)]
pub struct SkeletonRow {
    pub width: f32,
}

impl SkeletonRow {
    pub fn paint(&self, p: &mut Painter, rect: Rect) {
        p.quad(Quad::new(rect, metrics::ROW_RADIUS, p.palette().well));
        let cy = rect.y + rect.h * 0.5;
        let tile = Rect::new(rect.x + 8.0, cy - 8.0, 16.0, 16.0);
        p.quad(Quad::new(tile, 4.0, p.palette().badge).pattern(Pattern::Shimmer));
        let bar_x = tile.right() + 10.0;
        let bar = Rect::new(bar_x, cy - 2.5, (rect.right() - 12.0 - bar_x) * self.width, 5.0);
        p.quad(Quad::new(bar, 2.5, p.palette().badge).fill(Fill::solid(p.palette().badge)).pattern(Pattern::Shimmer));
    }
}
