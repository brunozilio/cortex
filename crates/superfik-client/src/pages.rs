//! Browser cards' pages as a shell sizes and drives them. A page renders
//! at the device scale, sized to its rect on the card (logical px); the
//! camera's zoom only scales its picture.
//!
//! [`RemotePages`] is a shell's side of pages that run on the server (ADR
//! 0010): the reducer's browser calls become `ClientCommand::Page`s, sized
//! as the card paints them, and the cursor each page asks for is kept.

use std::collections::HashMap;

use superfik_browser::Call;
use superfik_core::BrowserId;
use superfik_core::page::{PageCall, PageCursor, PageReport};
use superfik_protocol::ClientCommand;
use superfik_render::{Point, Rect};
use superfik_ui::theme::metrics;

pub fn page_size(rect: Rect, scale: f32) -> (u32, u32) {
    ((rect.w * scale).round().max(1.0) as u32, (rect.h * scale).round().max(1.0) as u32)
}

pub fn page_point(rect: Rect, at: Point, scale: f32) -> (i32, i32) {
    (((at.x - rect.x) * scale).round() as i32, ((at.y - rect.y) * scale).round() as i32)
}

pub fn nominal_page() -> Rect {
    Rect::new(0.0, 0.0, metrics::BROWSER_WIDTH - metrics::NODE_PADDING * 2.0, metrics::BROWSER_PAGE_HEIGHT)
}

#[derive(Debug, Default)]
pub struct PageSizes {
    sizes: HashMap<BrowserId, ((u32, u32), f32)>,
}

impl PageSizes {
    pub fn size(&mut self, call: Call, scale: f32) -> Call {
        match call {
            Call::Open { id, url, profile, .. } => {
                let size = page_size(nominal_page(), scale);
                self.sizes.insert(id, (size, scale));
                Call::Open { id, url, profile, size, scale }
            }
            Call::Close(id) => {
                self.sizes.remove(&id);
                Call::Close(id)
            }
            call => call,
        }
    }

    pub fn fit(&mut self, id: BrowserId, rect: Rect, scale: f32) -> Option<Call> {
        let size = page_size(rect, scale);
        let current = self.sizes.get_mut(&id)?;
        if *current == (size, scale) {
            return None;
        }
        *current = (size, scale);
        Some(Call::Resize { id, size, scale })
    }

    pub fn get(&self, id: BrowserId) -> Option<((u32, u32), f32)> {
        self.sizes.get(&id).copied()
    }

    pub fn is_open(&self, id: BrowserId) -> bool {
        self.sizes.contains_key(&id)
    }

    pub fn ids(&self) -> impl Iterator<Item = BrowserId> + '_ {
        self.sizes.keys().copied()
    }
}

#[derive(Debug, Default)]
pub struct RemotePages {
    pub sizes: PageSizes,
    cursors: HashMap<BrowserId, PageCursor>,
}

impl RemotePages {
    pub fn command(&mut self, call: Call, scale: f32) -> Option<ClientCommand> {
        let call = self.sizes.size(call, scale);
        if let Call::Close(id) = &call {
            self.cursors.remove(id);
        }
        call.remote().map(ClientCommand::Page)
    }

    pub fn fit(&mut self, id: BrowserId, rect: Rect, scale: f32) -> Option<ClientCommand> {
        let resize = self.sizes.fit(id, rect, scale)?;
        self.command(resize, scale)
    }

    pub fn reported(&mut self, id: BrowserId, report: &PageReport) {
        if let PageReport::Cursor(cursor) = report
            && self.sizes.is_open(id)
        {
            self.cursors.insert(id, *cursor);
        }
    }

    pub fn cursor(&self, id: BrowserId) -> Option<PageCursor> {
        self.cursors.get(&id).copied()
    }

    pub fn refresh(&self) -> Vec<ClientCommand> {
        self.sizes.ids().map(|id| ClientCommand::Page(PageCall::Refresh(id))).collect()
    }
}

#[cfg(test)]
mod tests {
    use superfik_browser::Profile;

    use super::*;

    #[test]
    fn pages_open_at_the_device_scale_and_resize_only_when_their_rect_changes() {
        let mut sizes = PageSizes::default();
        let id = BrowserId::new();
        let open = Call::Open { id, url: "about:blank".into(), profile: Profile::Ephemeral, size: (0, 0), scale: 1.0 };
        let expected = page_size(nominal_page(), 2.0);
        assert!(matches!(sizes.size(open, 2.0), Call::Open { size, scale: 2.0, .. } if size == expected));
        assert_eq!(sizes.fit(id, Rect::new(10.0, 10.0, nominal_page().w, nominal_page().h), 2.0), None);
        assert!(matches!(
            sizes.fit(id, Rect::new(10.0, 10.0, 300.0, 200.0), 2.0),
            Some(Call::Resize { size: (600, 400), .. })
        ));
        sizes.size(Call::Close(id), 2.0);
        assert_eq!(sizes.fit(id, Rect::new(10.0, 10.0, 100.0, 100.0), 2.0), None, "a closed page is not resized");
    }

    #[test]
    fn points_map_to_page_pixels() {
        let page = Rect::new(100.0, 50.0, 300.0, 200.0);
        assert_eq!(page_point(page, Point::new(110.0, 55.5), 2.0), (20, 11));
        assert_eq!(page_size(page, 1.5), (450, 300));
    }

    #[test]
    fn remote_pages_travel_as_commands_and_keep_their_cursor() {
        let mut pages = RemotePages::default();
        let id = BrowserId::new();
        let open =
            Call::Open { id, url: "https://a.test".into(), profile: Profile::Ephemeral, size: (0, 0), scale: 1.0 };
        assert!(matches!(pages.command(open, 2.0), Some(ClientCommand::Page(PageCall::Open { scale: 2.0, .. }))));
        assert_eq!(pages.command(Call::SetCookies { id, cookies: vec![] }, 2.0), None, "cookies stay home");
        pages.reported(id, &PageReport::Cursor(PageCursor::Pointer));
        assert_eq!(pages.cursor(id), Some(PageCursor::Pointer));
        assert_eq!(pages.refresh(), [ClientCommand::Page(PageCall::Refresh(id))]);
        assert!(matches!(pages.command(Call::Close(id), 2.0), Some(ClientCommand::Page(PageCall::Close(_)))));
        assert_eq!(pages.cursor(id), None);
        assert!(pages.refresh().is_empty());
    }
}
