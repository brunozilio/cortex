//! The header of the full-window screens (review, plugins): below the
//! window's traffic lights, a back chip at the left edge, the screen's
//! title beside it and its actions laid right to left from the right edge.

use superfik_render::{Point, Rect};

use crate::components::Chip;
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy)]
pub struct ScreenHeader<'a> {
    pub top_inset: f32,
    pub back: &'a str,
    pub back_hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenHeaderLayout {
    pub bar: Rect,
    pub back: Rect,
    pub center_y: f32,
    pub title_x: f32,
    pub actions_right: f32,
}

impl ScreenHeader<'_> {
    pub fn bar(area: Rect, top_inset: f32) -> Rect {
        Rect::new(area.x, area.y + top_inset, area.w, metrics::SCREEN_HEADER)
    }

    pub fn body(area: Rect, top_inset: f32) -> Rect {
        let top = Self::bar(area, top_inset).bottom();
        let pad = metrics::SCREEN_PADDING;
        Rect::new(area.x + pad, top, area.w - pad * 2.0, area.bottom() - top - pad)
    }

    pub fn paint(&self, p: &mut Painter, area: Rect) -> ScreenHeaderLayout {
        let bar = Self::bar(area, self.top_inset);
        let center_y = bar.center().y;
        let chip =
            Chip { icon: icons::ARROW_LEFT, label: self.back, active: false, hovered: self.back_hovered, tint: None };
        let back = chip.paint(p, Point::new(bar.x + metrics::SCREEN_PADDING, center_y - metrics::CHIP_HEIGHT * 0.5));
        ScreenHeaderLayout {
            bar,
            back,
            center_y,
            title_x: back.right() + metrics::SCREEN_TITLE_GAP,
            actions_right: bar.right() - metrics::SCREEN_PADDING,
        }
    }

    pub fn title(p: &mut Painter, layout: &ScreenHeaderLayout, title: &str, right: f32) {
        let width = (right - metrics::SCREEN_TITLE_GAP - layout.title_x).max(0.0);
        p.label_fit(title, typography::MODEL, layout.title_x, layout.center_y, width, p.palette().text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_starts_under_the_bar_inside_the_padding() {
        let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
        let bar = ScreenHeader::bar(area, metrics::TITLEBAR_HEIGHT);
        let body = ScreenHeader::body(area, metrics::TITLEBAR_HEIGHT);
        assert_eq!(bar.y, metrics::TITLEBAR_HEIGHT);
        assert_eq!(body.y, bar.bottom());
        assert_eq!(body.x, metrics::SCREEN_PADDING);
        assert_eq!(body.right(), 1200.0 - metrics::SCREEN_PADDING);
    }

    #[test]
    fn native_window_controls_reserve_no_client_space() {
        let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
        assert_eq!(ScreenHeader::bar(area, 0.0).y, 0.0);
        assert_eq!(ScreenHeader::body(area, 0.0).y, metrics::SCREEN_HEADER);
    }
}
