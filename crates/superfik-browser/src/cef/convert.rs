//! Our input and events ↔ welding's. Pure.

use welding::{
    CursorShape, EventModifiers, KeyEvent, KeyEventKind, MouseAction, MouseButton, MouseEvent, NavigationEvent,
};

use super::native_keys;
use crate::{KeyInput, KeyPhase, Modifiers, PageCursor, PageEvent, PageInput, PointerButton};

const ABORTED: i32 = -3;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    pub left: bool,
    pub middle: bool,
    pub right: bool,
}

impl Held {
    fn set(&mut self, button: PointerButton, down: bool) {
        match button {
            PointerButton::Left => self.left = down,
            PointerButton::Middle => self.middle = down,
            PointerButton::Right => self.right = down,
        }
    }
}

fn modifiers(m: Modifiers, held: Held) -> EventModifiers {
    EventModifiers {
        shift: m.shift,
        ctrl: m.control,
        alt: m.alt,
        meta: m.command,
        left_mouse_button: held.left,
        middle_mouse_button: held.middle,
        right_mouse_button: held.right,
    }
}

fn button(b: PointerButton) -> MouseButton {
    match b {
        PointerButton::Left => MouseButton::Left,
        PointerButton::Middle => MouseButton::Middle,
        PointerButton::Right => MouseButton::Right,
    }
}

pub fn mouse(input: &PageInput, held: &mut Held) -> Option<MouseEvent> {
    let event = |x, y, b, action, m| MouseEvent { x, y, button: b, action, modifiers: m };
    Some(match *input {
        PageInput::Move { x, y, modifiers: m } => {
            event(x, y, MouseButton::Left, MouseAction::Moved, modifiers(m, *held))
        }
        PageInput::Press { x, y, button: b, modifiers: m } => {
            held.set(b, true);
            event(x, y, button(b), MouseAction::Pressed, modifiers(m, *held))
        }
        PageInput::Release { x, y, button: b, modifiers: m } => {
            let event = event(x, y, button(b), MouseAction::Released, modifiers(m, *held));
            held.set(b, false);
            event
        }
        PageInput::Wheel { x, y, dx, dy, modifiers: m } => {
            event(x, y, MouseButton::Left, MouseAction::WheelScrolled { delta_x: dx, delta_y: dy }, modifiers(m, *held))
        }
        PageInput::Leave | PageInput::Key(_) | PageInput::Text(_) | PageInput::Edit(_) => return None,
    })
}

pub fn keys(key: &KeyInput) -> Vec<KeyEvent> {
    let character = key.text.or_else(|| native_keys::function_char(key.code));
    let event = |kind, character| KeyEvent {
        kind,
        windows_key_code: key.code,
        native_key_code: native_keys::native_code(key.code),
        character,
        modifiers: modifiers(key.modifiers, Held::default()),
    };
    match key.phase {
        KeyPhase::Up => vec![event(KeyEventKind::KeyUp, character)],
        KeyPhase::Down => {
            let mut events = vec![event(KeyEventKind::RawKeyDown, character)];
            if let Some(c) = key.text.filter(|_| !key.modifiers.command) {
                events.push(event(KeyEventKind::Char, Some(c)));
            }
            events
        }
    }
}

pub fn text(text: &str) -> Vec<KeyEvent> {
    text.chars()
        .map(|c| KeyEvent {
            kind: KeyEventKind::Char,
            windows_key_code: 0,
            native_key_code: 0,
            character: Some(c),
            modifiers: EventModifiers::default(),
        })
        .collect()
}

pub fn cursor(shape: CursorShape) -> PageCursor {
    match shape {
        CursorShape::Pointer => PageCursor::Pointer,
        CursorShape::Text => PageCursor::Text,
        CursorShape::Wait => PageCursor::Wait,
        CursorShape::Progress => PageCursor::Progress,
        CursorShape::Crosshair => PageCursor::Crosshair,
        CursorShape::Move | CursorShape::ResizeAll => PageCursor::Move,
        CursorShape::NotAllowed => PageCursor::NotAllowed,
        CursorShape::Help => PageCursor::Help,
        CursorShape::ResizeNs => PageCursor::ResizeNs,
        CursorShape::ResizeEw => PageCursor::ResizeEw,
        CursorShape::ResizeNeSw => PageCursor::ResizeNeSw,
        CursorShape::ResizeNwSe => PageCursor::ResizeNwSe,
        CursorShape::Grab => PageCursor::Grab,
        CursorShape::Grabbing => PageCursor::Grabbing,
        CursorShape::ZoomIn => PageCursor::ZoomIn,
        CursorShape::ZoomOut => PageCursor::ZoomOut,
        _ => PageCursor::Default,
    }
}

pub fn cookie(cookie: &crate::import::Cookie) -> welding::Cookie {
    use crate::import::SameSite;
    let domain = if cookie.domain.starts_with('.') { cookie.domain.clone() } else { String::new() };
    welding::Cookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain,
        path: cookie.path.clone(),
        secure: cookie.secure,
        http_only: cookie.http_only,
        same_site: cookie.same_site.map(|s| match s {
            SameSite::Strict => welding::SameSite::Strict,
            SameSite::Lax => welding::SameSite::Lax,
            SameSite::None => welding::SameSite::None,
        }),
        expires: cookie.expires,
        partitioned: false,
    }
}

#[derive(Debug, Default)]
pub struct Loads {
    started: Option<String>,
}

impl Loads {
    pub fn event(&mut self, event: NavigationEvent) -> Vec<PageEvent> {
        match &event {
            NavigationEvent::LoadStart { url } => self.started = Some(url.clone()),
            NavigationEvent::LoadEnd { url, http_status: ABORTED }
                if self.started.as_ref().is_some_and(|s| s != url) =>
            {
                return Vec::new();
            }
            _ => {}
        }
        self::event(event)
    }
}

pub fn event(event: NavigationEvent) -> Vec<PageEvent> {
    match event {
        NavigationEvent::LoadStart { .. } => vec![PageEvent::Loading(true)],
        NavigationEvent::LoadEnd { .. } => vec![PageEvent::Loading(false)],
        NavigationEvent::LoadError { error_code: ABORTED, .. } => Vec::new(),
        NavigationEvent::LoadError { url, error_text, .. } => {
            vec![PageEvent::Failed { url, error: error_text }, PageEvent::Loading(false)]
        }
        NavigationEvent::TitleChanged { title } => vec![PageEvent::Title(title)],
        NavigationEvent::AddressChanged { url } => vec![PageEvent::Address(url)],
        NavigationEvent::ContentProcessTerminated { .. } => vec![PageEvent::Crashed],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_carries_the_held_button() {
        let mut held = Held::default();
        let m = Modifiers::default();
        let press = PageInput::Press { x: 1, y: 2, button: PointerButton::Left, modifiers: m };
        assert!(mouse(&press, &mut held).is_some_and(|e| e.modifiers.left_mouse_button));
        let moved = mouse(&PageInput::Move { x: 5, y: 2, modifiers: m }, &mut held);
        assert!(moved.is_some_and(|e| e.modifiers.left_mouse_button && e.action == MouseAction::Moved));
        let release = PageInput::Release { x: 5, y: 2, button: PointerButton::Left, modifiers: m };
        assert!(mouse(&release, &mut held).is_some());
        assert!(!held.left);
        assert!(mouse(&PageInput::Leave, &mut held).is_none());
    }

    #[test]
    fn typing_sends_a_character_but_shortcuts_do_not() {
        let mut key = KeyInput { phase: KeyPhase::Down, code: 0x41, text: Some('a'), modifiers: Modifiers::default() };
        let kinds: Vec<KeyEventKind> = keys(&key).iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![KeyEventKind::RawKeyDown, KeyEventKind::Char]);
        key.modifiers.command = true;
        assert_eq!(keys(&key).len(), 1);
        key.phase = KeyPhase::Up;
        assert_eq!(keys(&key)[0].kind, KeyEventKind::KeyUp);
        assert_eq!(text("hé").len(), 2);
    }

    #[test]
    fn host_cookies_stay_on_their_host_and_domain_cookies_keep_their_domain() {
        let brought = |domain: &str| crate::import::Cookie {
            name: "__Host-GAPS".into(),
            value: "v".into(),
            domain: domain.into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            same_site: None,
            expires: None,
        };
        assert_eq!(cookie(&brought("accounts.google.com")).domain, "");
        assert_eq!(cookie(&brought(".google.com")).domain, ".google.com");
    }

    #[test]
    fn cursors_keep_their_css_names_and_unknown_ones_are_the_arrow() {
        assert_eq!(cursor(CursorShape::Pointer), PageCursor::Pointer);
        assert_eq!(cursor(CursorShape::ResizeAll), PageCursor::Move);
        assert_eq!(cursor(CursorShape::Custom("none".into())), PageCursor::Default);
    }

    #[test]
    fn the_end_of_a_load_cut_short_does_not_stop_the_next_one() {
        let start = |url: &str| NavigationEvent::LoadStart { url: url.into() };
        let end = |url: &str, http_status| NavigationEvent::LoadEnd { url: url.into(), http_status };
        let mut loads = Loads::default();
        assert_eq!(loads.event(start("old")), vec![PageEvent::Loading(true)]);
        assert_eq!(loads.event(start("new")), vec![PageEvent::Loading(true)]);
        assert!(loads.event(end("old", ABORTED)).is_empty());
        assert_eq!(loads.event(end("new", 200)), vec![PageEvent::Loading(false)]);
        loads.event(start("file"));
        assert_eq!(loads.event(end("file", ABORTED)), vec![PageEvent::Loading(false)]);
    }

    #[test]
    fn aborted_loads_are_not_failures() {
        let aborted = NavigationEvent::LoadError { url: "a".into(), error_code: ABORTED, error_text: "x".into() };
        assert!(event(aborted).is_empty());
        let failed = NavigationEvent::LoadError { url: "a".into(), error_code: -105, error_text: "dns".into() };
        assert_eq!(event(failed)[0], PageEvent::Failed { url: "a".into(), error: "dns".into() });
    }
}
