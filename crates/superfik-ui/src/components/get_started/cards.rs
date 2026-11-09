//! Get Started's feature cards: a raised surface holding a grained tile
//! with the feature's glyph in its accent, its name and a few lines about
//! it. A card grows from a pill at its tile, like a node from its
//! connector, and its content resolves once the surface is mostly there.

use std::sync::Arc;

use superfik_render::{Fill, Pattern, Point, Quad, Rect, TextLayout};

use super::Feature;
use crate::painter::Painter;
use crate::theme::{colors, metrics, motion, typography};

pub struct Card {
    pub body: Arc<TextLayout>,
    pub height: f32,
}

impl Card {
    pub fn measure(p: &mut Painter, feature: &Feature, width: f32) -> Self {
        let pad = metrics::GET_STARTED_CARD_PADDING;
        let text_w = (width - pad * 2.0 - metrics::GET_STARTED_TILE - metrics::GET_STARTED_TEXT_GAP).max(0.0);
        let body = p.layout(feature.body, typography::GET_STARTED_BODY, Some(text_w));
        let inner = (typography::GET_STARTED_FEATURE.line_height + body.height).max(metrics::GET_STARTED_TILE);
        Self { body, height: inner + pad * 2.0 }
    }
}

pub fn grown(full: Rect, t: f32) -> Rect {
    let (w, h) = metrics::GET_STARTED_CARD_PILL;
    let cx = full.x + metrics::GET_STARTED_CARD_PADDING + metrics::GET_STARTED_TILE * 0.5;
    let pill = Rect::new(cx - w * 0.5, full.center().y - h * 0.5, w, h);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Rect::new(lerp(pill.x, full.x), lerp(pill.y, full.y), lerp(pill.w, full.w), lerp(pill.h, full.h))
}

pub fn paint(p: &mut Painter, feature: &Feature, card: &Card, full: Rect, grow: f32, content: f32) {
    if grow <= 0.0 {
        return;
    }
    let rect = grown(full, grow);
    p.faded((grow * 4.0).min(1.0), |p| {
        p.surface_rect(rect, metrics::NODE_RADIUS.min(rect.h * 0.5), metrics::GET_STARTED_CARD_SMOOTH);
    });
    if content <= 0.0 {
        return;
    }
    let left = 1.0 - content;
    p.faded(content, |p| {
        p.blurred(left * motion::BLUR, |p| paint_content(p, feature, card, full.translate(0.0, left * 4.0)));
    });
}

fn paint_content(p: &mut Painter, feature: &Feature, card: &Card, full: Rect) {
    let c = p.palette();
    let pad = metrics::GET_STARTED_CARD_PADDING;
    let size = metrics::GET_STARTED_TILE;
    let tile = Rect::new(full.x + pad, full.center().y - size * 0.5, size, size);
    p.quad(
        Quad::new(tile, metrics::GET_STARTED_TILE_RADIUS, c.role_tile_top)
            .fill(Fill::vertical(c.role_tile_top, c.role_tile_bottom))
            .pattern(Pattern::Grain { amount: colors::ROLE_TILE_GRAIN }),
    );
    p.icon_at(feature.icon, tile.center(), metrics::GET_STARTED_GLYPH, Some(feature.tint));
    let text_x = tile.right() + metrics::GET_STARTED_TEXT_GAP;
    let line = typography::GET_STARTED_FEATURE.line_height;
    let top = full.center().y - (line + card.body.height) * 0.5;
    let title = p.layout(feature.title, typography::GET_STARTED_FEATURE, None);
    p.text(&title, Point::new(text_x, top), c.text);
    p.text(&card.body, Point::new(text_x, top + line), c.text_secondary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_grows_from_a_pill_at_its_tile() {
        let full = Rect::new(100.0, 200.0, 520.0, 88.0);
        assert_eq!(grown(full, 1.0), full);
        let pill = grown(full, 0.0);
        assert_eq!((pill.w, pill.h), metrics::GET_STARTED_CARD_PILL);
        let tile_x = full.x + metrics::GET_STARTED_CARD_PADDING + metrics::GET_STARTED_TILE * 0.5;
        assert!((pill.center().x - tile_x).abs() < 0.01);
        assert!((pill.center().y - full.center().y).abs() < 0.01);
    }
}
