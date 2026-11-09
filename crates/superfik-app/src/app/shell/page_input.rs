//! Pointer and keys for a browser card's page.
//!
//! The card's page rect is recorded while painting (canvas coordinates);
//! input over it is translated to page pixels and sent to the page, which
//! renders at the window's device scale.

use superfik_browser::{PageInput, PointerButton};
use winit::event::{KeyEvent, MouseButton, TouchPhase};
use winit::keyboard::PhysicalKey;

use super::App;
use crate::app::page_keys;
use crate::app::page_swipe::Swiped;
use crate::state::Action;
use superfik_client::pages::page_point;

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Left),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Right => Some(PointerButton::Right),
        _ => None,
    }
}

impl App {
    fn device_scale(&self) -> f32 {
        self.gpu.as_ref().map_or(1.0, |g| g.window.scale_factor() as f32)
    }

    fn page_target(&self, held: bool) -> Option<(superfik_core::BrowserId, (i32, i32))> {
        let ((id, rect), at) = match self.hits.expanded_page {
            Some(page) => (page, self.pointer.position),
            None => (self.hits.browser_page?, self.canvas_pointer()),
        };
        (held || rect.contains(at)).then(|| (id, page_point(rect, at, self.device_scale())))
    }

    fn shown_page(&self) -> Option<superfik_core::BrowserId> {
        self.hits.expanded_page.or(self.hits.browser_page).map(|(id, _)| id)
    }

    pub(super) fn page_cursor(&self) -> Option<winit::window::CursorIcon> {
        let (id, _) = self.page_target(self.page_pressed.is_some())?;
        self.pages.cursor(id).map(page_keys::cursor)
    }

    fn page_modifiers(&self) -> superfik_browser::Modifiers {
        page_keys::modifiers(self.modifiers)
    }

    pub(super) fn page_pointer_moved(&mut self) {
        let modifiers = self.page_modifiers();
        match self.page_target(self.page_pressed.is_some()) {
            Some((id, (x, y))) => {
                self.over_page = true;
                self.pages.input(id, PageInput::Move { x, y, modifiers });
            }
            None if self.over_page => {
                self.over_page = false;
                if let Some(id) = self.shown_page() {
                    self.pages.input(id, PageInput::Leave);
                }
            }
            None => {}
        }
    }

    pub(super) fn page_press(&mut self, button: MouseButton) {
        let (Some(b), Some((id, (x, y)))) = (pointer_button(button), self.page_target(false)) else { return };
        if self.session.browser().is_none_or(|card| card.id != id) {
            return;
        }
        self.page_pressed = Some(button);
        let modifiers = self.page_modifiers();
        self.pages.input(id, PageInput::Press { x, y, button: b, modifiers });
    }

    pub(super) fn page_release(&mut self, button: MouseButton) {
        if self.page_pressed != Some(button) {
            return;
        }
        self.page_pressed = None;
        let (Some(b), Some((id, (x, y)))) = (pointer_button(button), self.page_target(true)) else { return };
        let modifiers = self.page_modifiers();
        self.pages.input(id, PageInput::Release { x, y, button: b, modifiers });
    }

    pub(super) fn page_wheel(&mut self, dx: f32, dy: f32, trackpad: Option<TouchPhase>) -> bool {
        let Some((id, (x, y))) = self.page_target(false) else { return false };
        if let Some(swiped) = trackpad.and_then(|phase| self.page_swipe.feed(phase, dx, dy, self.now())) {
            self.dispatch(match swiped {
                Swiped::Back => Action::BrowserBack,
                Swiped::Forward => Action::BrowserForward,
            });
        }
        let scale = self.device_scale();
        let (dx, dy) = ((dx * scale).round() as i32, (dy * scale).round() as i32);
        let modifiers = self.page_modifiers();
        self.pages.input(id, PageInput::Wheel { x, y, dx, dy, modifiers });
        true
    }

    pub(super) fn page_key(&mut self, event: &KeyEvent) {
        let Some(card) = self.session.browser() else { return };
        let PhysicalKey::Code(code) = event.physical_key else { return };
        let id = card.id;
        let paste = || self.clipboard.get_text().ok();
        if event.state.is_pressed()
            && let Some(command) = page_keys::edit_command(code, self.shortcut_modifiers(), paste)
        {
            return self.pages.input(id, PageInput::Edit(command));
        }
        if let Some(key) = page_keys::key(code, event.state, event.text.as_deref(), self.modifiers) {
            self.pages.input(card.id, PageInput::Key(key));
        }
    }

    pub(super) fn page_text(&mut self, text: String) {
        if let Some(card) = self.session.browser() {
            self.pages.input(card.id, PageInput::Text(text));
        }
    }
}
