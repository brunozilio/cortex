//! The brand tile: a plugin's or provider's artwork on a rounded tile.

use superfik_render::{Fill, Point, Quad, Rect};

use crate::Painter;
use crate::brands::Brand;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy)]
pub struct BrandTile<'a> {
    pub brand: Brand,
    pub name: &'a str,
}

impl BrandTile<'_> {
    pub fn paint(&self, p: &mut Painter, rect: Rect) {
        if self.brand.own_colours
            && let Some(mark) = self.brand.mark
        {
            p.icon(mark, rect, None);
            return;
        }
        let c = self.brand.color;
        let white = superfik_render::Color::hex(0xffffff);
        let radius = rect.w * metrics::STORE_TILE_RADIUS;
        p.quad(
            Quad::new(rect, radius, c)
                .fill(Fill { top: c.mix(white, 0.14), bottom: c })
                .border(c.mix(superfik_render::Color::hex(0x000000), 0.25).faded(0.5), 1.0),
        );
        match self.brand.mark {
            Some(mark) => {
                p.icon_at(mark, rect.center(), rect.w * metrics::STORE_MARK, Some(white));
            }
            None => {
                let initial = match self.brand.monogram {
                    Some(letters) => letters.to_owned(),
                    None => self.name.chars().take(1).collect::<String>().to_uppercase(),
                };
                let size = rect.w * metrics::STORE_MONOGRAM;
                let style = superfik_render::TextStyle::new(size, size * 1.2, superfik_render::FontWeight::SemiBold);
                let layout = p.layout(&initial, style, None);
                let origin = Point::new(rect.center().x - layout.width * 0.5, rect.center().y - layout.height * 0.5);
                p.text(&layout, origin, white);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::colors;
    use superfik_render::{Color, Scene, TextSystem};

    #[test]
    fn official_artwork_fills_the_tile_without_an_extra_frame() {
        for palette in [&colors::DARK, &colors::LIGHT] {
            let mut scene = Scene::new(Color::TRANSPARENT, crate::painter::LAYERS);
            let mut text = TextSystem::new();
            let rect = Rect::new(10.0, 20.0, 36.0, 36.0);
            let mut painter = Painter::new(&mut scene, &mut text, 2.0, palette);
            BrandTile { brand: crate::brands::brand("github"), name: "GitHub" }.paint(&mut painter, rect);
            assert!(scene.layers.iter().all(|layer| layer.quads.is_empty()));
            let marks: Vec<_> = scene.layers.iter().flat_map(|layer| &layer.icons).collect();
            assert_eq!(marks.len(), 1);
            assert_eq!(marks[0].rect, rect);
            assert_eq!(marks[0].tint, None);
        }
    }
}
