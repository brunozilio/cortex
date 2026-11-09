//! Keys for a browser page: winit keys to Chromium's, and the cursors it
//! asks for to winit's. Pure and tested.

use superfik_browser::{EditCommand, KeyInput, KeyPhase, Modifiers, PageCursor};
use winit::event::ElementState;
use winit::keyboard::{KeyCode, ModifiersState};
use winit::window::CursorIcon;

pub fn modifiers(mods: ModifiersState) -> Modifiers {
    Modifiers { shift: mods.shift_key(), control: mods.control_key(), alt: mods.alt_key(), command: mods.super_key() }
}

pub fn key(code: KeyCode, state: ElementState, text: Option<&str>, mods: ModifiersState) -> Option<KeyInput> {
    let code = virtual_key(code)?;
    let phase = if state.is_pressed() { KeyPhase::Down } else { KeyPhase::Up };
    let text = text.and_then(|t| t.chars().next()).filter(|c| !c.is_control() || *c == '\r');
    Some(KeyInput { phase, code, text, modifiers: modifiers(mods) })
}

pub fn cursor(cursor: PageCursor) -> CursorIcon {
    match cursor {
        PageCursor::Default => CursorIcon::Default,
        PageCursor::Pointer => CursorIcon::Pointer,
        PageCursor::Text => CursorIcon::Text,
        PageCursor::Wait => CursorIcon::Wait,
        PageCursor::Progress => CursorIcon::Progress,
        PageCursor::Crosshair => CursorIcon::Crosshair,
        PageCursor::Move => CursorIcon::Move,
        PageCursor::NotAllowed => CursorIcon::NotAllowed,
        PageCursor::Help => CursorIcon::Help,
        PageCursor::ResizeNs => CursorIcon::NsResize,
        PageCursor::ResizeEw => CursorIcon::EwResize,
        PageCursor::ResizeNeSw => CursorIcon::NeswResize,
        PageCursor::ResizeNwSe => CursorIcon::NwseResize,
        PageCursor::Grab => CursorIcon::Grab,
        PageCursor::Grabbing => CursorIcon::Grabbing,
        PageCursor::ZoomIn => CursorIcon::ZoomIn,
        PageCursor::ZoomOut => CursorIcon::ZoomOut,
    }
}

pub fn edit_command(
    code: KeyCode,
    mods: ModifiersState,
    paste: impl FnOnce() -> Option<String>,
) -> Option<EditCommand> {
    if !mods.super_key() {
        return None;
    }
    Some(match code {
        KeyCode::KeyC => EditCommand::Copy,
        KeyCode::KeyX => EditCommand::Cut,
        KeyCode::KeyV => EditCommand::Paste(paste()?),
        KeyCode::KeyA => EditCommand::SelectAll,
        KeyCode::KeyZ if mods.shift_key() => EditCommand::Redo,
        KeyCode::KeyZ => EditCommand::Undo,
        _ => return None,
    })
}

fn virtual_key(code: KeyCode) -> Option<i32> {
    use KeyCode as K;
    if let Some(letter) = letter_code(code) {
        return Some(letter);
    }
    Some(match code {
        K::Backspace => 0x08,
        K::Tab => 0x09,
        K::Enter | K::NumpadEnter => 0x0D,
        K::ShiftLeft | K::ShiftRight => 0x10,
        K::ControlLeft | K::ControlRight => 0x11,
        K::AltLeft | K::AltRight => 0x12,
        K::Escape => 0x1B,
        K::Space => 0x20,
        K::PageUp => 0x21,
        K::PageDown => 0x22,
        K::End => 0x23,
        K::Home => 0x24,
        K::ArrowLeft => 0x25,
        K::ArrowUp => 0x26,
        K::ArrowRight => 0x27,
        K::ArrowDown => 0x28,
        K::Delete => 0x2E,
        K::Digit0 => 0x30,
        K::Digit1 => 0x31,
        K::Digit2 => 0x32,
        K::Digit3 => 0x33,
        K::Digit4 => 0x34,
        K::Digit5 => 0x35,
        K::Digit6 => 0x36,
        K::Digit7 => 0x37,
        K::Digit8 => 0x38,
        K::Digit9 => 0x39,
        K::SuperLeft => 0x5B,
        K::SuperRight => 0x5C,
        K::F1 => 0x70,
        K::F2 => 0x71,
        K::F3 => 0x72,
        K::F4 => 0x73,
        K::F5 => 0x74,
        K::F6 => 0x75,
        K::F7 => 0x76,
        K::F8 => 0x77,
        K::F9 => 0x78,
        K::F10 => 0x79,
        K::F11 => 0x7A,
        K::F12 => 0x7B,
        K::Semicolon => 0xBA,
        K::Equal => 0xBB,
        K::Comma => 0xBC,
        K::Minus => 0xBD,
        K::Period => 0xBE,
        K::Slash => 0xBF,
        K::Backquote => 0xC0,
        K::BracketLeft => 0xDB,
        K::Backslash => 0xDC,
        K::BracketRight => 0xDD,
        K::Quote => 0xDE,
        _ => return None,
    })
}

fn letter_code(code: KeyCode) -> Option<i32> {
    const LETTERS: [KeyCode; 26] = {
        use KeyCode as K;
        [
            K::KeyA,
            K::KeyB,
            K::KeyC,
            K::KeyD,
            K::KeyE,
            K::KeyF,
            K::KeyG,
            K::KeyH,
            K::KeyI,
            K::KeyJ,
            K::KeyK,
            K::KeyL,
            K::KeyM,
            K::KeyN,
            K::KeyO,
            K::KeyP,
            K::KeyQ,
            K::KeyR,
            K::KeyS,
            K::KeyT,
            K::KeyU,
            K::KeyV,
            K::KeyW,
            K::KeyX,
            K::KeyY,
            K::KeyZ,
        ]
    };
    LETTERS.iter().position(|&l| l == code).map(|i| 0x41 + i as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_shortcuts_become_edit_commands() {
        let cmd = ModifiersState::SUPER;
        let clip = || Some("x".to_owned());
        assert_eq!(edit_command(KeyCode::KeyC, cmd, clip), Some(EditCommand::Copy));
        assert_eq!(edit_command(KeyCode::KeyV, cmd, clip), Some(EditCommand::Paste("x".into())));
        assert_eq!(edit_command(KeyCode::KeyV, cmd, || None), None, "nothing to paste");
        assert_eq!(edit_command(KeyCode::KeyZ, cmd | ModifiersState::SHIFT, clip), Some(EditCommand::Redo));
        assert_eq!(edit_command(KeyCode::KeyC, ModifiersState::empty(), clip), None, "a plain c types");
    }

    #[test]
    fn page_cursors_become_the_windows() {
        assert_eq!(cursor(PageCursor::Pointer), CursorIcon::Pointer);
        assert_eq!(cursor(PageCursor::Text), CursorIcon::Text);
        assert_eq!(cursor(PageCursor::Grabbing), CursorIcon::Grabbing);
    }

    #[test]
    fn letters_and_named_keys_get_windows_codes() {
        let down = ElementState::Pressed;
        let none = ModifiersState::empty();
        let a = key(KeyCode::KeyA, down, Some("a"), none).expect("a");
        assert_eq!((a.code, a.text, a.phase), (0x41, Some('a'), KeyPhase::Down));
        assert_eq!(key(KeyCode::KeyZ, down, Some("z"), none).map(|k| k.code), Some(0x5A));
        let enter = key(KeyCode::Enter, down, Some("\r"), none).expect("enter");
        assert_eq!((enter.code, enter.text), (0x0D, Some('\r')), "Enter submits forms");
        let tab = key(KeyCode::Tab, down, Some("\t"), none).expect("tab");
        assert_eq!(tab.text, None, "Tab moves the focus, it types nothing");
        let up = key(KeyCode::ArrowUp, ElementState::Released, None, ModifiersState::SHIFT).expect("up");
        assert_eq!((up.code, up.phase, up.modifiers.shift), (0x26, KeyPhase::Up, true));
        assert!(key(KeyCode::Fn, down, None, none).is_none());
    }
}
