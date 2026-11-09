//! A list of choices painted inside a popover.

use superfik_render::{Color, Icon, Rect};

use super::row::{Row, Trailing};
use crate::painter::Painter;
use crate::theme::typography;

#[derive(Debug, Clone, Copy)]
pub struct MenuItem<'a> {
    pub icon: Option<(Icon, Option<Color>)>,
    pub label: &'a str,
    pub selected: bool,
    pub badge: Option<&'a str>,
    pub muted: bool,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { icon: None, label, selected: false, badge: None, muted: false }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Menu<'a> {
    pub items: &'a [MenuItem<'a>],
    pub hovered: Option<usize>,
}

impl Menu<'_> {
    pub const ROW: f32 = 35.0;
    pub const PADDING: f32 = 4.0;

    pub fn height(count: usize) -> f32 {
        count as f32 * Self::ROW + Self::PADDING * 2.0
    }

    pub fn paint(&self, p: &mut Painter, panel: Rect) -> Vec<Rect> {
        let mut rows = Vec::with_capacity(self.items.len());
        for (i, item) in self.items.iter().enumerate() {
            let rect = Rect::new(
                panel.x + Self::PADDING,
                panel.y + Self::PADDING + i as f32 * Self::ROW,
                panel.w - Self::PADDING * 2.0,
                Self::ROW,
            );
            let hovered = self.hovered == Some(i) && !item.muted;
            let mut row = Row::new(item.label).style(typography::MENU).bare().hovered(hovered).muted(item.muted);
            if let Some((icon, tint)) = item.icon {
                row = row.icon(icon, tint);
            }
            if item.selected {
                row = row.trailing(Trailing::Check);
            } else if let Some(badge) = item.badge {
                row = row.trailing(Trailing::Count(badge));
            }
            rows.push(row.paint(p, rect));
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_fits_rows_and_padding() {
        assert_eq!(Menu::height(4), 148.0);
    }
}
