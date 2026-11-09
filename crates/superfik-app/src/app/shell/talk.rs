//! Holding ⌘ (either one) to talk: the shell feeds `PushToTalk` the key's
//! presses and releases, any other key or click, focus loss and the time,
//! and dispatches what it decides. macOS only; elsewhere the modifier is
//! Ctrl, too busy to talk with.

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use superfik_client::input::push_to_talk::TalkKey;

use super::App;

impl App {
    pub(super) fn talk_key(&mut self, event: &KeyEvent) {
        if !cfg!(target_os = "macos") || !self.session.offers_voice() {
            return;
        }
        let talk = matches!(event.physical_key, PhysicalKey::Code(KeyCode::SuperLeft | KeyCode::SuperRight));
        let now = self.now();
        let action = match (talk, event.state) {
            (true, ElementState::Pressed) => {
                let action = self.push_to_talk.press(TalkKey::Talk, now);
                self.request_redraw();
                action
            }
            (true, ElementState::Released) => self.push_to_talk.release(TalkKey::Talk),
            (false, ElementState::Pressed) => self.push_to_talk.press(TalkKey::Other, now),
            (false, ElementState::Released) => None,
        };
        if let Some(action) = action {
            self.dispatch(action);
        }
    }

    pub(super) fn talk_other(&mut self) {
        let now = self.now();
        if let Some(action) = self.push_to_talk.press(TalkKey::Other, now) {
            self.dispatch(action);
        }
    }

    pub(super) fn talk_tick(&mut self) {
        let now = self.now();
        if let Some(action) = self.push_to_talk.tick(now) {
            self.dispatch(action);
        } else if self.push_to_talk.is_waiting() {
            self.request_redraw();
        }
    }

    pub(super) fn talk_blur(&mut self) {
        if let Some(action) = self.push_to_talk.blur() {
            self.dispatch(action);
        }
    }
}
