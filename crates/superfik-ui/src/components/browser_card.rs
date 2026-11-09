//! The browser card: one toolbar row (back, forward, reload, the address,
//! the vault key, private, expand) over the page. The page arrives as an image
//! the app keeps up to date.

use superfik_render::{ImageId, Point, Quad, Rect};

use super::node::{EarSide, NodeFrame, busy_dots};
use crate::icons;
use crate::painter::Painter;
use crate::text_edit::TextEdit;
use crate::theme::{metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPart {
    Back,
    Forward,
    Reload,
    Address,
    Page,
    Logins,
    Private,
    Expand,
    Header,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageImage {
    pub image: ImageId,
    pub size: (f32, f32),
    pub fading: Option<FadingFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FadingFrame {
    pub image: ImageId,
    pub size: (f32, f32),
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct BrowserCard<'a> {
    pub rect: Rect,
    pub title: &'a str,
    pub site: &'a str,
    pub address: &'a TextEdit,
    pub placeholder: &'a str,
    pub editing: bool,
    pub caret_visible: bool,
    pub loading: bool,
    pub back: bool,
    pub forward: bool,
    pub private: bool,
    pub logins: bool,
    pub logins_open: bool,
    pub expanded: bool,
    pub turn: f32,
    pub page: Option<PageImage>,
    pub failure: Option<&'a str>,
    pub hovered: Option<BrowserPart>,
    pub agent: Option<&'a str>,
    pub ear_hovered: bool,
    pub content: f32,
    pub time: f32,
}

#[derive(Debug, Clone, Default)]
pub struct BrowserCardHits {
    pub parts: Vec<(Rect, BrowserPart)>,
    pub page: Rect,
    pub knob: Option<Rect>,
}

impl BrowserCard<'_> {
    pub fn height() -> f32 {
        metrics::NODE_HEADER + metrics::BROWSER_PAGE_HEIGHT + metrics::NODE_PADDING
    }

    pub fn paint(&self, p: &mut Painter) -> BrowserCardHits {
        let mut frame = NodeFrame::new(self.rect, icons::GLOBE, None, self.title);
        frame.plain = true;
        frame.ear = (!self.expanded).then_some(EarSide::Right);
        frame.ear_hovered = self.ear_hovered;
        frame.content = self.content;
        let knob = frame.paint(p).knob;
        let page = Self::page_rect(self.rect);
        let mut parts = vec![(Self::toolbar(self.rect), BrowserPart::Header)];
        p.faded(self.content, |p| {
            self.paint_toolbar(p, &mut parts);
            self.page(p, page);
        });
        parts.push((page, BrowserPart::Page));
        BrowserCardHits { parts, page, knob }
    }

    pub fn toolbar(rect: Rect) -> Rect {
        Rect::new(rect.x, rect.y, rect.w, metrics::NODE_HEADER)
    }

    pub fn page_rect(rect: Rect) -> Rect {
        let pad = metrics::NODE_PADDING;
        let top = rect.y + metrics::NODE_HEADER;
        Rect::new(rect.x + pad, top, rect.w - pad * 2.0, rect.bottom() - pad - top)
    }

    fn page(&self, p: &mut Painter, rect: Rect) {
        p.quad(Quad::new(rect, metrics::WELL_RADIUS, p.palette().well));
        match (self.failure, self.page) {
            (Some(failure), _) => {
                let center = rect.center();
                p.icon_at(
                    icons::GLOBE,
                    Point::new(center.x, center.y - 14.0),
                    metrics::ICON_LARGE,
                    Some(p.palette().icon_muted),
                );
                let layout = p.layout(failure, typography::LABEL, None);
                let at = Point::new(center.x - layout.width * 0.5, center.y + 2.0);
                p.text(&layout, at, p.palette().text_secondary);
            }
            (None, Some(_)) if self.loading => {}
            (None, Some(page)) => p.clipped(rect, |p| {
                frame(p, rect, page.image, page.size);
                if let Some(old) = page.fading {
                    p.faded(old.opacity, |p| frame(p, rect, old.image, old.size));
                }
            }),
            (None, None) => {}
        }
        if self.loading && self.failure.is_none() {
            loading(p, rect.center(), self.time);
        }
    }
}

fn frame(p: &mut Painter, rect: Rect, image: ImageId, size: (f32, f32)) {
    let scale = if size.0 > 0.0 { rect.w / size.0 } else { 1.0 };
    p.image(image, Rect::new(rect.x, rect.y, rect.w, size.1 * scale), metrics::WELL_RADIUS, 0.0);
}

fn loading(p: &mut Painter, center: Point, time: f32) {
    let (w, h) = metrics::BROWSER_LOADING;
    let pill = Rect::new(center.x - w * 0.5, center.y - h * 0.5, w, h);
    p.quad(Quad::new(pill, h * 0.5, p.palette().surface_top).border(p.palette().divider, 1.0));
    busy_dots(p, center.x, center.y, time);
}
