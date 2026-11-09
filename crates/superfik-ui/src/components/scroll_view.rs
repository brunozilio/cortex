//! A clipped viewport over taller content, with a thumb on the right. The
//! offset lives with the caller; this only clamps and paints.

use superfik_render::{Point, Quad, Rect};

use crate::painter::Painter;

#[derive(Debug, Clone, Copy)]
pub struct ScrollView {
    pub content_height: f32,
    pub offset: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollHits {
    pub viewport: Rect,
    pub thumb: Option<Rect>,
    pub offset: f32,
    pub max_offset: f32,
}

impl ScrollView {
    pub const THUMB_WIDTH: f32 = 4.0;
    const THUMB_MIN: f32 = 24.0;

    pub fn max_offset(content_height: f32, viewport_height: f32) -> f32 {
        (content_height - viewport_height).max(0.0)
    }

    pub fn paint<R>(
        &self,
        p: &mut Painter,
        viewport: Rect,
        f: impl FnOnce(&mut Painter, Point) -> R,
    ) -> (R, ScrollHits) {
        let max_offset = Self::max_offset(self.content_height, viewport.h);
        let offset = self.offset.clamp(0.0, max_offset);
        let origin = Point::new(viewport.x, viewport.y - offset);
        let result = p.clipped(viewport, |p| f(p, origin));
        let thumb = (max_offset > 0.0).then(|| {
            let track = viewport.h - 8.0;
            let height = (track * viewport.h / self.content_height).max(Self::THUMB_MIN).min(track);
            let y = viewport.y + 4.0 + (track - height) * (offset / max_offset);
            let rect = Rect::new(viewport.right() - Self::THUMB_WIDTH - 4.0, y, Self::THUMB_WIDTH, height);
            p.quad(Quad::new(rect, Self::THUMB_WIDTH * 0.5, p.palette().knob.faded(0.5)));
            rect
        });
        (result, ScrollHits { viewport, thumb, offset, max_offset })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_clamp_to_the_content() {
        assert_eq!(ScrollView::max_offset(100.0, 300.0), 0.0);
        assert_eq!(ScrollView::max_offset(1000.0, 300.0), 700.0);
    }
}
