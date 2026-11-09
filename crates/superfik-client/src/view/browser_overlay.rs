//! The browser card expanded over the whole window. Painted in window
//! coordinates (the overlay pass) so it sits above the canvas and the
//! sidebar; it grows out of its card and shrinks back into it, over a
//! scrim that swallows what is underneath.

use std::collections::HashMap;

use superfik_core::BrowserId;
use superfik_render::{Quad, Rect};
use superfik_ui::components::{BrowserCard, PageImage};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane};

use super::motions::Motions;
use super::nodes::browser::{Labels, props};
use super::target::{HitMap, Target};
use crate::state::Session;

pub struct ExpandedBrowserView<'a> {
    pub session: &'a Session,
    pub pages: &'a HashMap<BrowserId, PageImage>,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub caret_visible: bool,
    pub now: f64,
}

impl ExpandedBrowserView<'_> {
    pub fn rect(viewport: (f32, f32)) -> Rect {
        let inset = metrics::BROWSER_EXPANDED_INSET;
        let top = metrics::BROWSER_EXPANDED_TOP;
        Rect::new(inset, top, (viewport.0 - inset * 2.0).max(0.0), (viewport.1 - top - inset).max(0.0))
    }

    pub fn paint(&self, p: &mut Painter, motions: &mut Motions, card: Option<Rect>, hits: &mut HitMap) {
        let browser = self.session.browser();
        let open = browser.is_some_and(|b| b.expanded);
        motions.sync_expanded_browser(open, card, self.now);
        let (Some(browser), Some(grow)) = (browser, motions.expanded_browser(self.now)) else { return };
        let t = grow.progress(self.now);
        let target = Self::rect(self.viewport);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let from = grow.from;
        let rect =
            Rect::new(lerp(from.x, target.x), lerp(from.y, target.y), lerp(from.w, target.w), lerp(from.h, target.h));
        let window = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        let labels = Labels::new(&self.session.strings());
        let page = self.pages.get(&browser.id).copied();
        let mut view = props(browser, &labels, rect, page, self.hovered, self.caret_visible);
        view.expanded = true;
        view.turn = t;
        view.time = self.now as f32;
        let painted = p.on_plane(Plane::Overlay, |p| {
            p.quad(Quad::new(window, 0.0, p.palette().scrim.faded(t)));
            view.paint(p)
        });
        if !open {
            return;
        }
        hits.add_screen(window, Target::Surface);
        for (rect, part) in painted.parts {
            hits.add_screen(rect, Target::Browser(part));
        }
        hits.expanded_page = Some((browser.id, BrowserCard::page_rect(target)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_expanded_card_fills_the_window_inside_its_margins() {
        let rect = ExpandedBrowserView::rect((1440.0, 900.0));
        assert_eq!(rect.x, metrics::BROWSER_EXPANDED_INSET);
        assert_eq!(rect.right(), 1440.0 - metrics::BROWSER_EXPANDED_INSET);
        assert_eq!(rect.bottom(), 900.0 - metrics::BROWSER_EXPANDED_INSET);
        assert!(ExpandedBrowserView::rect((10.0, 10.0)).w >= 0.0);
    }

    #[test]
    fn it_grows_from_the_card_and_shrinks_back_into_it() {
        let mut motions = Motions::default();
        let card = Rect::new(400.0, 300.0, 680.0, 470.0);
        motions.sync_expanded_browser(true, Some(card), 0.0);
        let grow = motions.expanded_browser(0.0).expect("growing");
        assert_eq!((grow.from, grow.progress(0.0)), (card, 0.0));
        assert_eq!(motions.expanded_browser(5.0).map(|g| g.progress(5.0)), Some(1.0));
        motions.sync_expanded_browser(false, Some(card), 5.0);
        assert!(motions.expanded_browser(5.1).is_some(), "still shrinking");
        motions.sync_expanded_browser(false, None, 10.0);
        assert!(motions.expanded_browser(10.0).is_none(), "gone once back");
    }
}
