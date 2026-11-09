//! The browser card's toolbar, in the header's row: back, forward and
//! reload; the address (the site and the page's title until it is
//! edited); then the vault key (when logins belong to the page), private
//! and expand. Reload turns into a spinner while
//! the page loads; the expand arrows turn inward as the card grows.

use superfik_render::{Icon, Point, Quad, Rect};

use super::badge::Badge;
use super::browser_card::{BrowserCard, BrowserPart};
use super::browser_glyphs::{expand_arrows, spinner};
use super::text_field::{caret, caret_position, selection};
use crate::icons;
use crate::painter::Painter;
use crate::theme::{metrics, typography};

#[derive(Clone, Copy)]
enum Glyph {
    Icon(Icon),
    Arrows(f32),
    Spinner,
}

impl BrowserCard<'_> {
    pub(super) fn paint_toolbar(&self, p: &mut Painter, parts: &mut Vec<(Rect, BrowserPart)>) {
        let (size, gap) = (metrics::BROWSER_BUTTON, metrics::BROWSER_BUTTON_GAP);
        let cy = Self::toolbar(self.rect).center().y;
        let mut x = self.rect.x + metrics::HEADER_ICON_X - size * 0.5;
        let reload = if self.loading { Glyph::Spinner } else { Glyph::Icon(icons::RELOAD) };
        let leading = [
            (BrowserPart::Back, Glyph::Icon(icons::ARROW_LEFT), self.back),
            (BrowserPart::Forward, Glyph::Icon(icons::ARROW_RIGHT), self.forward),
            (BrowserPart::Reload, reload, true),
        ];
        for (part, glyph, enabled) in leading {
            self.toolbar_button(p, parts, Rect::new(x, cy - size * 0.5, size, size), part, glyph, enabled);
            x += size + gap;
        }
        let key = self.logins.then_some((BrowserPart::Logins, Glyph::Icon(icons::KEY)));
        let trailing =
            [(BrowserPart::Private, Glyph::Icon(icons::INCOGNITO)), (BrowserPart::Expand, Glyph::Arrows(self.turn))];
        for (part, glyph) in key.into_iter().chain(trailing) {
            self.toolbar_button(p, parts, Self::button(self.rect, part), part, glyph, true);
        }
        let first = if self.logins { BrowserPart::Logins } else { BrowserPart::Private };
        let mut right = Self::button(self.rect, first).x - metrics::BROWSER_ADDRESS_GAP;
        if let Some(text) = self.agent {
            right = Badge { text }.paint(p, right, cy).x - metrics::BROWSER_ADDRESS_GAP;
        }
        let left = x - gap + metrics::BROWSER_ADDRESS_GAP;
        let address = Rect::new(left, cy - size * 0.5, (right - left).max(0.0), size);
        self.paint_address(p, address);
        parts.push((address, BrowserPart::Address));
    }

    pub fn button(rect: Rect, part: BrowserPart) -> Rect {
        let (size, gap) = (metrics::BROWSER_BUTTON, metrics::BROWSER_BUTTON_GAP);
        let from_right = match part {
            BrowserPart::Logins => 2.0,
            BrowserPart::Private => 1.0,
            _ => 0.0,
        };
        let x = rect.right() - metrics::HEADER_ICON_X - size * 0.5 - from_right * (size + gap);
        let cy = Self::toolbar(rect).center().y;
        Rect::new(x, cy - size * 0.5, size, size)
    }

    fn toolbar_button(
        &self,
        p: &mut Painter,
        parts: &mut Vec<(Rect, BrowserPart)>,
        rect: Rect,
        part: BrowserPart,
        glyph: Glyph,
        enabled: bool,
    ) {
        let active =
            (part == BrowserPart::Private && self.private) || (part == BrowserPart::Logins && self.logins_open);
        let hovered = enabled && self.hovered == Some(part);
        if active || hovered {
            let fill = if active { p.palette().control_active } else { p.palette().hover };
            p.quad(Quad::new(rect, metrics::CHIP_RADIUS, fill));
        }
        let tint = if enabled { p.palette().icon } else { p.palette().icon_muted };
        let (center, size) = (rect.center(), metrics::ICON);
        match glyph {
            Glyph::Icon(icon) => p.icon_at(icon, center, size, Some(tint)),
            Glyph::Arrows(turn) => expand_arrows(p, center, size, turn, tint),
            Glyph::Spinner => spinner(p, center, size, self.time, tint),
        }
        if enabled {
            parts.push((rect, part));
        }
    }

    fn paint_address(&self, p: &mut Painter, rect: Rect) {
        let fill = if self.editing {
            Some(p.palette().well)
        } else {
            (self.hovered == Some(BrowserPart::Address)).then(|| p.palette().hover)
        };
        if let Some(fill) = fill {
            p.quad(Quad::new(rect, metrics::CHIP_RADIUS, fill));
        }
        let inset = metrics::BROWSER_ADDRESS_GAP;
        let cy = rect.center().y;
        let x = rect.x + inset;
        let room = rect.w - inset * 2.0;
        p.clipped(rect, |p| {
            if self.editing {
                return self.paint_edited(p, x, cy);
            }
            if self.site.is_empty() {
                let shown = if self.title.is_empty() { self.placeholder } else { self.title };
                return p.label_fit(shown, typography::MENU, x, cy, room, p.palette().placeholder);
            }
            let site = p.layout(self.site, typography::MODEL, None);
            p.label_fit(self.site, typography::MODEL, x, cy, room, p.palette().text);
            let rest = room - site.width;
            if !self.title.is_empty() && rest > 0.0 {
                let title = format!(" / {}", self.title);
                p.label_fit(&title, typography::MENU, x + site.width, cy, rest, p.palette().text_tertiary);
            }
        });
    }

    fn paint_edited(&self, p: &mut Painter, x: f32, cy: f32) {
        let style = typography::MENU;
        let top = (cy - style.line_height * 0.5).round();
        let origin = Point::new(x, top);
        let text = self.address.text();
        if text.is_empty() {
            p.label(self.placeholder, style, x, cy, p.palette().placeholder);
        } else {
            let layout = p.layout(text, style, None);
            if let Some((a, b)) = self.address.selection() {
                selection(p, &layout, origin, a, b, style.line_height);
            }
            p.text(&layout, origin, p.palette().text);
            if self.caret_visible {
                let (dx, dy) = caret_position(&layout, self.address.caret());
                caret(p, Point::new(x + dx, top + dy), style.line_height);
            }
            return;
        }
        if self.caret_visible {
            caret(p, origin, style.line_height);
        }
    }
}
