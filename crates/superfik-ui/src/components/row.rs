//! A carved row: leading icon, label and an optional trailing element.

use superfik_render::{Color, Icon, Quad, Rect, TextStyle};

use super::badge::Badge;
use super::knob::Knob;
use crate::icons;
use crate::painter::Painter;
use crate::theme::colors::Palette;
use crate::theme::{metrics, typography};

pub fn icon_tint(tint: Option<Color>, palette: &Palette) -> Color {
    tint.unwrap_or(palette.icon)
}

#[cfg(test)]
mod icon_tint_tests {
    use super::*;
    use crate::theme::colors::{DARK, LIGHT};

    #[test]
    fn an_untinted_row_icon_takes_the_palette_icon_colour_in_both_themes() {
        assert_eq!(icon_tint(None, &LIGHT), LIGHT.icon, "dark ink on light surfaces");
        assert_eq!(icon_tint(None, &DARK), DARK.icon);
        let branch = Color::hex(0x2fae3b);
        assert_eq!(icon_tint(Some(branch), &LIGHT), branch, "an explicit tint wins");
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trailing<'a> {
    None,
    Count(&'a str),
    Check,
    Caret,
    Remove { hovered: bool },
    Select(&'a str),
}

#[derive(Debug, Clone, Copy)]
pub struct Row<'a> {
    pub icon: Option<(Icon, Option<Color>)>,
    pub icon_size: f32,
    pub icon_gap: f32,
    pub label: &'a str,
    pub style: TextStyle,
    pub trailing: Trailing<'a>,
    pub hovered: bool,
    pub color: Option<Color>,
    pub well: bool,
    pub muted: bool,
}

impl<'a> Row<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            icon: None,
            icon_size: metrics::ICON,
            icon_gap: metrics::ROW_ICON_GAP,
            label,
            color: None,
            style: typography::LABEL,
            trailing: Trailing::None,
            hovered: false,
            well: true,
            muted: false,
        }
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    pub fn icon(mut self, icon: Icon, tint: Option<Color>) -> Self {
        self.icon = Some((icon, tint));
        self
    }

    pub fn icon_size(mut self, size: f32, gap: f32) -> Self {
        self.icon_size = size;
        self.icon_gap = gap;
        self
    }

    pub fn trailing(mut self, trailing: Trailing<'a>) -> Self {
        self.trailing = trailing;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    pub fn bare(mut self) -> Self {
        self.well = false;
        self
    }

    pub fn knob_rect(rect: Rect) -> Rect {
        let c = Self::knob_center(rect);
        let r = Knob::RADIUS + 4.0;
        Rect::new(c.x - r, c.y - r, r * 2.0, r * 2.0)
    }

    fn knob_center(rect: Rect) -> superfik_render::Point {
        superfik_render::Point::new(rect.right() - 8.0 - Knob::RADIUS, rect.y + rect.h * 0.5)
    }

    pub fn paint(&self, p: &mut Painter, rect: Rect) -> Rect {
        if self.well || self.hovered {
            let color = if self.hovered { p.palette().hover } else { p.palette().well };
            p.quad(Quad::new(rect, metrics::ROW_RADIUS, color));
        }
        let cy = rect.y + rect.h * 0.5;
        let mut x = rect.x + 10.0;
        if let Some((icon, tint)) = self.icon {
            let half = self.icon_size * 0.5;
            let tint = Some(icon_tint(tint, p.palette()));
            p.icon_at(icon, superfik_render::Point::new(x + half, cy), self.icon_size, tint);
            x += self.icon_size + self.icon_gap;
        }
        let right = rect.right() - 8.0;
        let trailing_width = match self.trailing {
            Trailing::None => 0.0,
            Trailing::Count(text) => Badge { text }.paint(p, right, cy).w + 6.0,
            Trailing::Check => {
                p.icon_at(
                    icons::CHECK_CIRCLE,
                    superfik_render::Point::new(right - 4.0, cy),
                    13.0,
                    Some(p.palette().icon),
                );
                20.0
            }
            Trailing::Caret => {
                p.icon_at(
                    icons::CARET_DOWN,
                    superfik_render::Point::new(right - 5.0, cy),
                    12.0,
                    Some(p.palette().icon_muted),
                );
                18.0
            }
            Trailing::Remove { hovered } => {
                Knob::close(Self::knob_center(rect), hovered).paint(p);
                Knob::RADIUS * 2.0 + 8.0
            }
            Trailing::Select(value) => {
                let caret = superfik_render::Point::new(right - 5.0, cy);
                p.icon_at(icons::CARET_DOWN, caret, 12.0, Some(p.palette().icon_muted));
                let room = (rect.w * metrics::ROW_SELECT_SHARE).max(0.0);
                let fitted = p.fit(value, typography::CAPTION, room);
                let w = p.layout(&fitted, typography::CAPTION, None).width;
                let layout = p.label(&fitted, typography::CAPTION, right - 18.0 - w, cy, p.palette().text_secondary);
                if fitted != value {
                    let y = (cy - layout.height * 0.5).round();
                    p.note_elided(Rect::new(right - 18.0 - w, y, w, layout.height), value);
                }
                18.0 + w + 8.0
            }
        };
        let max = (right - trailing_width - x).max(0.0);
        let color = if self.muted { p.palette().text_secondary } else { self.color.unwrap_or(p.palette().text_body) };
        p.label_fit(self.label, self.style, x, cy, max, color);
        rect
    }
}
