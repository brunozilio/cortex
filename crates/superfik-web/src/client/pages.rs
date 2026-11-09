//! Browser cards' pages for the web client. They run on the server (ADR
//! 0010): the reducer's browser calls go there as `ClientCommand::Page`
//! (`superfik_client::pages::RemotePages`), the frames that come back are
//! decoded for the page to paint, and the pointer, wheel and keys over a
//! page are sent to it, as the desktop sends them to its own pages.

use superfik_client::input::controller::app_owns_key;
use superfik_client::input::keys::KeyCommand;
use superfik_client::input::page_keys;
use superfik_client::pages::page_point;
use superfik_client::state::{BrowserFocus, EditKey, Effect};
use superfik_core::BrowserId;
use superfik_core::page::{EditCommand, KeyInput, PageCall, PageCursor, PageFrame, PageInput, PointerButton};
use superfik_frames::Decoded;
use superfik_protocol::ClientCommand;

use super::WebClient;
use crate::Io;

#[derive(Debug)]
pub struct PagePaint {
    pub id: BrowserId,
    pub size: (u32, u32),
    pub whole: bool,
    pub tiles: Vec<(u32, u32, Decoded)>,
}

fn send(call: PageCall) -> Io {
    Io::Send(Box::new(ClientCommand::Page(call)))
}

impl WebClient {
    pub(super) fn page_commands(&mut self, effects: Vec<Effect>) -> (Vec<Effect>, Vec<Io>) {
        let mut io = Vec::new();
        let mut rest = Vec::new();
        for effect in effects {
            match effect {
                Effect::Browser(call) => {
                    io.extend(self.pages.command(call, self.scale).map(|command| Io::Send(Box::new(command))));
                }
                effect => rest.push(effect),
            }
        }
        (rest, io)
    }

    pub(super) fn page_frame(&self, frame: PageFrame) -> Vec<Io> {
        if !self.pages.sizes.is_open(frame.id) {
            return Vec::new();
        }
        let tiles = frame
            .tiles
            .iter()
            .filter_map(|tile| superfik_frames::decode(tile).ok().map(|decoded| (tile.x, tile.y, decoded)))
            .collect();
        vec![Io::PaintPage(PagePaint { id: frame.id, size: frame.size, whole: frame.whole, tiles })]
    }

    pub(super) fn fit_page(&mut self) -> Vec<Io> {
        let shown = self.hits.expanded_page.or(self.hits.browser_page);
        let resize = shown.and_then(|(id, rect)| self.pages.fit(id, rect, self.scale));
        resize.map(|command| Io::Send(Box::new(command))).into_iter().collect()
    }

    fn page_target(&self, held: bool) -> Option<(BrowserId, (i32, i32))> {
        let at = self.pointer.position;
        let ((id, rect), at) = match self.hits.expanded_page {
            Some(page) => (page, at),
            None => (self.hits.browser_page?, self.camera.to_canvas(at)),
        };
        (held || rect.contains(at)).then(|| (id, page_point(rect, at, self.scale)))
    }

    pub(super) fn page_moved(&mut self) -> Vec<Io> {
        let modifiers = page_keys::modifiers(self.modifiers);
        match self.page_target(self.page_pressed) {
            Some((id, (x, y))) => {
                self.over_page = true;
                vec![send(PageCall::Input(id, PageInput::Move { x, y, modifiers }))]
            }
            None if self.over_page => {
                self.over_page = false;
                let shown = self.hits.expanded_page.or(self.hits.browser_page);
                shown.map(|(id, _)| send(PageCall::Input(id, PageInput::Leave))).into_iter().collect()
            }
            None => Vec::new(),
        }
    }

    pub(super) fn page_press(&mut self) -> Vec<Io> {
        let Some((id, (x, y))) = self.page_target(false) else { return Vec::new() };
        if self.session.browser().is_none_or(|card| card.id != id) {
            return Vec::new();
        }
        self.page_pressed = true;
        let modifiers = page_keys::modifiers(self.modifiers);
        vec![send(PageCall::Input(id, PageInput::Press { x, y, button: PointerButton::Left, modifiers }))]
    }

    pub(super) fn page_release(&mut self) -> Vec<Io> {
        if !std::mem::take(&mut self.page_pressed) {
            return Vec::new();
        }
        let Some((id, (x, y))) = self.page_target(true) else { return Vec::new() };
        let modifiers = page_keys::modifiers(self.modifiers);
        vec![send(PageCall::Input(id, PageInput::Release { x, y, button: PointerButton::Left, modifiers }))]
    }

    pub(super) fn page_wheel(&mut self, dx: f32, dy: f32) -> Option<Vec<Io>> {
        let (id, (x, y)) = self.page_target(false)?;
        let (dx, dy) = ((dx * self.scale).round() as i32, (dy * self.scale).round() as i32);
        let modifiers = page_keys::modifiers(self.modifiers);
        Some(vec![send(PageCall::Input(id, PageInput::Wheel { x, y, dx, dy, modifiers }))])
    }

    pub(super) fn for_page(&self, command: &KeyCommand) -> bool {
        self.session.browser_focus() == Some(BrowserFocus::Page) && !app_owns_key(command)
    }

    pub fn page_key(&mut self, key: KeyInput) -> Vec<Io> {
        let Some(card) = self.session.browser().filter(|_| self.page_has_keys()) else { return Vec::new() };
        vec![send(PageCall::Input(card.id, PageInput::Key(key)))]
    }

    pub(super) fn key_on_page(&mut self, command: KeyCommand, page: Option<KeyInput>) -> Vec<Io> {
        let edit = match command {
            KeyCommand::Copy => Some(EditCommand::Copy),
            KeyCommand::Cut => Some(EditCommand::Cut),
            KeyCommand::Undo => Some(EditCommand::Undo),
            KeyCommand::Redo => Some(EditCommand::Redo),
            _ => None,
        };
        match (edit, page, command) {
            (Some(edit), _, _) => self.page_edit(edit),
            (None, Some(key), _) => self.page_key(key),
            (None, None, KeyCommand::Edit(EditKey::Insert(text))) => self.page_text(text),
            _ => Vec::new(),
        }
    }

    fn page_edit(&mut self, edit: EditCommand) -> Vec<Io> {
        let Some(card) = self.session.browser() else { return Vec::new() };
        vec![send(PageCall::Input(card.id, PageInput::Edit(edit)))]
    }

    pub(super) fn page_text(&mut self, text: String) -> Vec<Io> {
        let Some(card) = self.session.browser() else { return Vec::new() };
        vec![send(PageCall::Input(card.id, PageInput::Text(text)))]
    }

    pub fn page_has_keys(&self) -> bool {
        self.session.browser_focus() == Some(BrowserFocus::Page)
    }

    pub fn page_cursor(&self) -> Option<PageCursor> {
        let (id, _) = self.page_target(self.page_pressed)?;
        self.pages.cursor(id)
    }

    pub fn page_open(&self, id: BrowserId) -> bool {
        self.pages.sizes.is_open(id)
    }
}
