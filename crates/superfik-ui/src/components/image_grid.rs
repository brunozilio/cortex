//! A three-column grid of reference tiles (image thumbnails or files) with
//! an add tile and a remove knob on the hovered tile. The tile open in the
//! viewer wears a ring; the viewer itself is `ImageViewer`.

use superfik_render::{ImageId, Point, Quad, Rect};

use super::knob::RemoveBadge;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tile<'a> {
    Image { id: ImageId, aspect: f32 },
    File { name: &'a str },
}

#[derive(Debug, Clone)]
pub struct ImageGrid<'a> {
    pub tiles: &'a [Tile<'a>],
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub hovered_remove: Option<usize>,
    pub hovered_add: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ImageGridHits {
    pub tiles: Vec<Rect>,
    pub removes: Vec<Option<Rect>>,
    pub add: Rect,
}

impl ImageGrid<'_> {
    pub const COLUMNS: usize = 3;
    pub const GAP: f32 = 6.0;
    pub const RADIUS: f32 = 6.0;

    pub fn tile_size(width: f32) -> f32 {
        (width - Self::GAP * (Self::COLUMNS - 1) as f32) / Self::COLUMNS as f32
    }

    pub fn rows(count: usize) -> usize {
        (count + 1).div_ceil(Self::COLUMNS).max(2)
    }

    pub fn height(width: f32, count: usize) -> f32 {
        let rows = Self::rows(count) as f32;
        rows * Self::tile_size(width) + (rows - 1.0) * Self::GAP
    }

    pub fn paint(&self, p: &mut Painter, area: Rect) -> ImageGridHits {
        let mut hits = ImageGridHits::default();
        let size = Self::tile_size(area.w);
        let cells = Self::rows(self.tiles.len()) * Self::COLUMNS;
        for cell in 0..cells {
            let (col, row) = ((cell % Self::COLUMNS) as f32, (cell / Self::COLUMNS) as f32);
            let rect = Rect::new(area.x + col * (size + Self::GAP), area.y + row * (size + Self::GAP), size, size);
            if let Some(tile) = self.tiles.get(cell) {
                self.paint_tile(p, cell, *tile, rect, &mut hits);
            } else if cell == self.tiles.len() {
                let color = if self.hovered_add { p.palette().hover } else { p.palette().well };
                p.quad(Quad::new(rect, Self::RADIUS, color));
                p.icon_at(icons::PLUS_CIRCLE, rect.center(), 14.0, Some(p.palette().icon));
                hits.add = rect;
            } else {
                p.quad(Quad::new(rect, Self::RADIUS, p.palette().well));
            }
        }
        hits
    }

    fn paint_tile(&self, p: &mut Painter, index: usize, tile: Tile, rect: Rect, hits: &mut ImageGridHits) {
        let selected = self.selected == Some(index);
        let hovered = self.hovered == Some(index) || self.hovered_remove == Some(index);
        let mut well = Quad::new(rect, Self::RADIUS, if hovered { p.palette().hover } else { p.palette().well });
        if selected {
            well = well.border(p.palette().reference, 1.5);
        }
        p.quad(well);
        match tile {
            Tile::Image { id, .. } => p.image(id, rect.inset(if selected { 2.0 } else { 0.0 }), Self::RADIUS, 0.0),
            Tile::File { name } => {
                p.icon_at(
                    icons::DOCUMENT,
                    Point::new(rect.center().x, rect.center().y - 7.0),
                    20.0,
                    Some(p.palette().icon),
                );
                let ext = extension(name);
                let layout = p.layout(&ext, typography::CAPTION, None);
                let x = (rect.center().x - layout.width * 0.5).round();
                p.label(&ext, typography::CAPTION, x, rect.bottom() - 11.0, p.palette().text_secondary);
            }
        }
        hits.tiles.push(rect);
        let knob = hovered.then(|| {
            let inset = metrics::TILE_REMOVE_RADIUS + metrics::TILE_REMOVE_INSET;
            let center = Point::new(rect.right() - inset, rect.y + inset);
            RemoveBadge { center, hovered: self.hovered_remove == Some(index) }.paint(p)
        });
        hits.removes.push(knob);
    }
}

fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => ext.to_ascii_uppercase(),
        _ => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_always_has_room_for_the_add_tile() {
        assert_eq!(ImageGrid::rows(0), 2);
        assert_eq!(ImageGrid::rows(3), 2);
        assert_eq!(ImageGrid::rows(5), 2);
        assert_eq!(ImageGrid::rows(6), 3);
    }

    #[test]
    fn tiles_fill_the_width() {
        let size = ImageGrid::tile_size(196.0);
        assert!((size * 3.0 + ImageGrid::GAP * 2.0 - 196.0).abs() < 1e-4);
    }

    #[test]
    fn height_is_the_rows_alone() {
        let size = ImageGrid::tile_size(196.0);
        assert!((ImageGrid::height(196.0, 3) - (2.0 * size + ImageGrid::GAP)).abs() < 1e-4);
    }

    #[test]
    fn file_tiles_show_their_extension() {
        assert_eq!(extension("notes.md"), "MD");
        assert_eq!(extension("Makefile"), "Makefile");
        assert_eq!(extension(".env"), ".env");
    }
}
