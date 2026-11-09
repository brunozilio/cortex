//! Browser cards' pages in the shell: the host that runs them, their
//! latest frames on the GPU, and the size each page renders at.
//!
//! A page renders at the window's device scale, sized to its rect on the
//! card (logical px); the camera's zoom only scales the texture.

use std::collections::HashMap;

use superfik_browser::import::BrowserKind;
use superfik_browser::{BrowserHost, Call, PageCursor, PageEvent, PageInput, Profile};
use superfik_client::pages::PageSizes;
use superfik_core::BrowserId;
use superfik_render::{Rect, Renderer};
use superfik_ui::components::PageImage;

use super::cookie_clearing::CookieClearing;
use super::logins::Logins;
use super::page_frames::PageFrames;
use crate::state::Action;

pub struct Pages {
    host: Box<dyn BrowserHost>,
    clipboard: super::clipboard::Clipboard,
    logins: Logins,
    frames: PageFrames,
    sizes: PageSizes,
    cursors: HashMap<BrowserId, PageCursor>,
    profiles: HashMap<BrowserId, Profile>,
    clearing: CookieClearing,
    submissions: super::login_fill::Submissions,
}

impl std::fmt::Debug for Pages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pages").field("pages", &self.profiles.len()).finish_non_exhaustive()
    }
}

impl Pages {
    pub fn new(host: Box<dyn BrowserHost>, cache_root: std::path::PathBuf) -> Self {
        Self {
            host,
            clipboard: super::clipboard::Clipboard::default(),
            logins: Logins::new(cache_root.clone()),
            frames: PageFrames::default(),
            sizes: PageSizes::default(),
            cursors: HashMap::new(),
            profiles: HashMap::new(),
            clearing: CookieClearing::new(cache_root),
            submissions: super::login_fill::Submissions::default(),
        }
    }

    pub fn clipboard(&self) -> super::clipboard::Clipboard {
        self.clipboard.clone()
    }

    pub fn save_login(&mut self, browser: BrowserId, account: String) -> Option<superfik_protocol::ClientCommand> {
        self.submissions.save(browser, account)
    }

    pub fn clear_cookies(&mut self) {
        let open: Vec<(BrowserId, Profile)> = self.profiles.iter().map(|(id, p)| (*id, p.clone())).collect();
        self.clearing.clear(&open).into_iter().for_each(|call| self.host.send(call));
        self.logins.forget();
    }

    pub fn import_logins(&mut self, kind: BrowserKind) {
        self.logins.import(kind);
    }

    pub fn follow_logins(&mut self, from: Option<BrowserKind>) {
        self.logins.follow(from);
    }

    pub fn send(&mut self, call: Call, scale: f32) {
        let call = match self.sizes.size(call, scale) {
            Call::Open { id, url, profile, size, scale } => {
                let cookies: Vec<Call> =
                    self.clearing.opened(id, &profile).into_iter().chain(self.logins.opened(id, &profile)).collect();
                self.profiles.insert(id, profile.clone());
                let again = (!cookies.is_empty()).then(|| Call::Navigate(id, url.clone()));
                self.host.send(Call::Open { id, url, profile, size, scale });
                cookies.into_iter().chain(again).for_each(|call| self.host.send(call));
                return;
            }
            Call::Close(id) => {
                self.profiles.remove(&id);
                Call::Close(id)
            }
            call => call,
        };
        self.host.send(call);
    }

    pub fn warm_up(&mut self) {
        self.host.warm_up();
    }

    pub fn input(&mut self, id: BrowserId, input: PageInput) {
        self.host.send(Call::Input(id, input));
    }

    pub fn fit(&mut self, id: BrowserId, rect: Rect, scale: f32) {
        if let Some(resize) = self.sizes.fit(id, rect, scale) {
            self.host.send(resize);
        }
    }

    pub fn is_active(&self) -> bool {
        self.host.is_active() || self.logins.is_busy()
    }

    pub fn suspend(&mut self) {
        self.host.suspend();
    }

    pub fn pump(&mut self, renderer: &mut Renderer) -> (Vec<Action>, bool) {
        let shown: Vec<(BrowserId, Profile)> = self.profiles.iter().map(|(id, p)| (*id, p.clone())).collect();
        let (calls, imported) = self.logins.poll(&shown);
        calls.into_iter().for_each(|call| self.host.send(call));
        let (device, queue) = renderer.device();
        let mut events = self.host.pump(device, queue);
        self.submissions.keep(&mut events);
        events.retain(|(id, event)| match event {
            PageEvent::Copied(text) => {
                let _ = self.clipboard.set_text(text.clone());
                false
            }
            PageEvent::Cursor(cursor) => {
                self.cursors.insert(*id, *cursor);
                false
            }
            _ => true,
        });
        for (id, event) in &events {
            tracing::debug!(%id, ?event, "page");
        }
        let mut painted = !events.is_empty();
        let open: Vec<BrowserId> = self.sizes.ids().collect();
        for id in open {
            let Some(frame) = self.host.frame(id) else { continue };
            let Some((asked, scale)) = self.sizes.get(id) else { continue };
            painted |= self.frames.show(renderer, id, &frame, asked, scale);
        }
        painted |= self.frames.tick(renderer);
        let sizes = &self.sizes;
        self.frames.release_closed(renderer, |id| sizes.is_open(*id));
        self.cursors.retain(|id, _| sizes.is_open(*id));
        self.submissions.retain(|id| sizes.is_open(*id));
        let mut actions: Vec<Action> = imported.into_iter().collect();
        actions.extend(events.into_iter().map(|(id, event)| Action::BrowserPage(id, event)));
        (actions, painted)
    }

    pub fn ids(&self) -> &HashMap<BrowserId, PageImage> {
        self.frames.ids()
    }

    pub fn cursor(&self, id: BrowserId) -> Option<PageCursor> {
        self.cursors.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use superfik_browser::{Profile, Recorder};

    use super::*;

    #[derive(Debug, Default, Clone)]
    struct Shared(std::rc::Rc<std::cell::RefCell<Recorder>>);

    impl BrowserHost for Shared {
        fn send(&mut self, call: Call) {
            self.0.borrow_mut().send(call);
        }
        fn pump(
            &mut self,
            device: &superfik_browser::wgpu::Device,
            queue: &superfik_browser::wgpu::Queue,
        ) -> Vec<(BrowserId, superfik_browser::PageEvent)> {
            self.0.borrow_mut().pump(device, queue)
        }
        fn frame(&self, id: BrowserId) -> Option<superfik_browser::Frame> {
            self.0.borrow().frame(id)
        }
        fn is_active(&self) -> bool {
            self.0.borrow().is_active()
        }
    }

    #[test]
    fn pages_open_sized_and_resize_only_when_their_rect_changes() {
        let shared = Shared::default();
        let mut pages = Pages::new(Box::new(shared.clone()), std::path::PathBuf::from("/nowhere"));
        let id = BrowserId::new();
        let open = Call::Open { id, url: "about:blank".into(), profile: Profile::Ephemeral, size: (0, 0), scale: 1.0 };
        pages.send(open, 2.0);
        assert!(matches!(shared.0.borrow().calls[..], [Call::Open { scale: 2.0, .. }]));
        pages.fit(id, Rect::new(10.0, 10.0, 300.0, 200.0), 2.0);
        assert!(matches!(shared.0.borrow().calls[1], Call::Resize { size: (600, 400), .. }));
        pages.fit(id, Rect::new(10.0, 10.0, 300.0, 200.0), 2.0);
        assert_eq!(shared.0.borrow().calls.len(), 2, "same size: nothing to do");
    }

    #[test]
    fn a_page_signed_in_as_it_opens_loads_its_address_again_with_the_cookies() {
        let shared = Shared::default();
        let mut pages = Pages::new(Box::new(shared.clone()), std::path::PathBuf::from("/nowhere"));
        let id = BrowserId::new();
        let url = "https://github.com".to_owned();
        pages.logins.follow(Some(BrowserKind::Dia));
        pages.logins.seed_for_test(BrowserKind::Dia);
        let profile = Profile::Persistent("new".into());
        pages.send(Call::Open { id, url: url.clone(), profile, size: (0, 0), scale: 1.0 }, 2.0);
        let calls = &shared.0.borrow().calls;
        assert!(
            matches!(&calls[..], [Call::Open { .. }, Call::SetCookies { .. }, Call::Navigate(to, again)] if *to == id && *again == url),
            "{calls:?}"
        );
    }
}
