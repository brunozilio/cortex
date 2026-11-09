//! The tooltip that shows a shortened label's whole text: a small raised
//! card under the pointer, kept inside the window.

use superfik_render::{Point, Quad, Rect, Shadow};

use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct Tooltip<'a> {
    pub text: &'a str,
    pub pointer: Point,
    pub viewport: Rect,
    pub progress: f32,
}

impl Tooltip<'_> {
    pub fn paint(&self, p: &mut Painter) -> Rect {
        let max = (self.viewport.w - metrics::TOOLTIP_MARGIN * 2.0 - metrics::TOOLTIP_PADDING.0 * 2.0)
            .clamp(0.0, metrics::TOOLTIP_MAX_WIDTH);
        let layout = p.layout(self.text, typography::TOOLTIP, Some(max));
        let (pad_x, pad_y) = metrics::TOOLTIP_PADDING;
        let size = (layout.width.ceil() + pad_x * 2.0, layout.height + pad_y * 2.0);
        let card = place(size, self.pointer, self.viewport);
        let t = self.progress.clamp(0.0, 1.0);
        p.faded(t, |p| {
            let c = p.palette();
            let shadow = Shadow { color: c.tooltip_shadow, offset_y: 2.0, blur: 8.0 };
            p.quad(Quad::new(card, metrics::TOOLTIP_RADIUS, c.tooltip).shadow(shadow));
            p.text(&layout, Point::new(card.x + pad_x, card.y + pad_y), c.text);
        });
        card
    }
}

fn place(size: (f32, f32), pointer: Point, viewport: Rect) -> Rect {
    let margin = metrics::TOOLTIP_MARGIN;
    let right = viewport.right() - margin - size.0;
    let x = pointer.x.min(right).max(viewport.x + margin);
    let below = pointer.y + metrics::TOOLTIP_GAP;
    let y = if below + size.1 <= viewport.bottom() - margin {
        below
    } else {
        (pointer.y - metrics::TOOLTIP_GAP * 0.5 - size.1).max(viewport.y + margin)
    };
    Rect::new(x.round(), y.round(), size.0, size.1)
}

#[cfg(test)]
mod tests {
    use superfik_render::{Color, Scene, TextSystem};

    use super::*;
    use crate::painter::LAYERS;
    use crate::theme::colors;

    const WINDOW: Rect = Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 };

    #[test]
    fn hangs_below_and_right_of_the_pointer() {
        let card = place((100.0, 26.0), Point::new(200.0, 100.0), WINDOW);
        assert_eq!((card.x, card.y), (200.0, 100.0 + metrics::TOOLTIP_GAP));
    }

    #[test]
    fn stays_inside_the_window_near_its_edges() {
        let card = place((300.0, 26.0), Point::new(780.0, 590.0), WINDOW);
        assert!(card.right() <= WINDOW.right() - metrics::TOOLTIP_MARGIN);
        assert!(card.bottom() < 590.0, "flips above the pointer: {card:?}");
    }

    #[test]
    fn long_text_wraps_within_the_widest_line() {
        let mut scene = Scene::new(Color::TRANSPARENT, LAYERS);
        let mut text = TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &colors::DARK);
        let long = "a very long label ".repeat(20);
        let tip = Tooltip { text: &long, pointer: Point::new(10.0, 10.0), viewport: WINDOW, progress: 1.0 };
        let card = tip.paint(&mut p);
        assert!(card.w <= metrics::TOOLTIP_MAX_WIDTH + metrics::TOOLTIP_PADDING.0 * 2.0 + 1.0, "{card:?}");
        assert!(card.h > typography::TOOLTIP.line_height * 2.0, "wraps onto several lines: {card:?}");
    }
}
