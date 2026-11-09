//! Full-window viewer for one reference. A scrim dims the whole window and
//! the tile grows from where it sits in its grid to the largest fit inside
//! the window's margins; closing runs the same path back. Painted in
//! window coordinates, over everything.

use superfik_render::{Point, Quad, Rect, Shadow};

use super::image_grid::{ImageGrid, Tile};
use super::knob::Knob;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, motion, typography};

#[derive(Debug, Clone)]
pub struct ImageViewer<'a> {
    pub tile: Tile<'a>,
    pub from: Rect,
    pub viewport: Rect,
    pub progress: f32,
    pub close_hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageViewerHits {
    pub scrim: Rect,
    pub frame: Rect,
    pub close: Rect,
}

impl ImageViewer<'_> {
    pub fn open_frame(viewport: Rect, tile: Tile) -> Rect {
        match tile {
            Tile::Image { aspect, .. } => fit(viewport.inset(metrics::VIEWER_MARGIN), aspect),
            Tile::File { .. } => {
                let side = metrics::VIEWER_FILE.min(viewport.w - 2.0 * metrics::VIEWER_MARGIN).max(0.0);
                let c = viewport.center();
                Rect::new(c.x - side * 0.5, c.y - side * 0.5, side, side)
            }
        }
    }

    pub fn frame(&self) -> Rect {
        lerp_rect(self.from, Self::open_frame(self.viewport, self.tile), self.progress.clamp(0.0, 1.0))
    }

    pub fn paint(&self, p: &mut Painter) -> ImageViewerHits {
        let t = self.progress.clamp(0.0, 1.0);
        p.quad(Quad::new(self.viewport, 0.0, p.palette().scrim).opacity(t));
        let frame = self.frame();
        let radius = lerp(ImageGrid::RADIUS, metrics::VIEWER_RADIUS, t);
        let shadow = Shadow { color: p.palette().viewer_shadow.faded(t), offset_y: 12.0 * t, blur: 32.0 * t };
        p.quad(Quad::new(frame, radius, p.palette().well).shadow(shadow));
        match self.tile {
            Tile::Image { id, .. } => p.image(id, frame, radius, motion::BLUR * (1.0 - t)),
            Tile::File { name } => p.faded(t, |p| {
                let c = frame.center();
                p.icon_at(icons::DOCUMENT, Point::new(c.x, c.y - 12.0), 28.0, Some(p.palette().icon));
                let text = p.fit(name, typography::LABEL, frame.w - 20.0);
                let layout = p.layout(&text, typography::LABEL, None);
                let x = (c.x - layout.width * 0.5).round();
                p.label(&text, typography::LABEL, x, c.y + 16.0, p.palette().text_secondary);
                if text != name {
                    let y = (c.y + 16.0 - layout.height * 0.5).round();
                    p.note_elided(Rect::new(x, y, layout.width, layout.height), name);
                }
            }),
        }
        let inset = metrics::VIEWER_CLOSE_INSET;
        let center = Point::new(self.viewport.right() - inset, self.viewport.y + inset);
        let close = p.faded(t, |p| Knob::close(center, self.close_hovered).paint(p));
        ImageViewerHits { scrim: self.viewport, frame, close }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.w, b.w, t), lerp(a.h, b.h, t))
}

pub fn fit(bounds: Rect, aspect: f32) -> Rect {
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
    let (w, h) =
        if bounds.w / bounds.h > aspect { (bounds.h * aspect, bounds.h) } else { (bounds.w, bounds.w / aspect) };
    Rect::new(bounds.x + (bounds.w - w) * 0.5, bounds.y + (bounds.h - h) * 0.5, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use superfik_render::ImageId;

    const VIEWPORT: Rect = Rect::new(0.0, 0.0, 1200.0, 800.0);
    const TILE: Rect = Rect::new(40.0, 300.0, 60.0, 60.0);

    fn image(aspect: f32) -> Tile<'static> {
        Tile::Image { id: ImageId(0), aspect }
    }

    fn viewer(tile: Tile<'_>, progress: f32) -> ImageViewer<'_> {
        ImageViewer { tile, from: TILE, viewport: VIEWPORT, progress, close_hovered: false }
    }

    #[test]
    fn grows_from_the_tile_to_the_window() {
        assert_eq!(viewer(image(2.0), 0.0).frame(), TILE);
        let open = viewer(image(2.0), 1.0).frame();
        let bounds = VIEWPORT.inset(metrics::VIEWER_MARGIN);
        assert!((open.w / open.h - 2.0).abs() < 1e-4, "keeps its aspect");
        assert!(open.x >= bounds.x && open.right() <= bounds.right() + 1e-3);
        assert!(open.y >= bounds.y && open.bottom() <= bounds.bottom() + 1e-3);
        assert_eq!(open.center(), bounds.center());
        let half = viewer(image(2.0), 0.5).frame();
        assert!(half.x > TILE.x && half.x < open.x);
    }

    #[test]
    fn files_open_into_a_centred_square() {
        let open = ImageViewer::open_frame(VIEWPORT, Tile::File { name: "notes.md" });
        assert_eq!((open.w, open.h), (metrics::VIEWER_FILE, metrics::VIEWER_FILE));
        assert_eq!(open.center(), VIEWPORT.center());
    }

    #[test]
    fn images_fit_without_stretching() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let wide = fit(bounds, 2.0);
        assert_eq!((wide.w, wide.h, wide.y), (100.0, 50.0, 25.0));
        let tall = fit(bounds, 0.5);
        assert_eq!((tall.w, tall.h, tall.x), (50.0, 100.0, 25.0));
        assert_eq!(fit(bounds, 0.0), bounds, "bad aspects fill the square");
    }
}
