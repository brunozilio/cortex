//! A DOM `KeyboardEvent` as winit's key, so keys typed into the page's
//! hidden text field (`textarea`, which gives the browser's input methods
//! something to compose into) go through the desktop's own mapping
//! (`input::keys::map_key`). Pure.

use superfik_client::input::page_keys;
use superfik_core::page::KeyInput;
use winit::event::ElementState;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, SmolStr};

pub fn key(name: &str, mods: ModifiersState) -> Option<(Key, Option<String>)> {
    let named = match name {
        "Enter" => NamedKey::Enter,
        "Backspace" => NamedKey::Backspace,
        "Delete" => NamedKey::Delete,
        "Tab" => NamedKey::Tab,
        "Escape" => NamedKey::Escape,
        "ArrowLeft" => NamedKey::ArrowLeft,
        "ArrowRight" => NamedKey::ArrowRight,
        "ArrowUp" => NamedKey::ArrowUp,
        "ArrowDown" => NamedKey::ArrowDown,
        "Home" => NamedKey::Home,
        "End" => NamedKey::End,
        " " => return Some((Key::Named(NamedKey::Space), Some(" ".to_owned()))),
        _ if name.chars().count() == 1 => {
            let text = (!mods.super_key() && !mods.control_key()).then(|| name.to_owned());
            return Some((Key::Character(SmolStr::new(name)), text));
        }
        _ => return None,
    };
    Some((Key::Named(named), None))
}

pub fn modifiers(shift: bool, control: bool, alt: bool, meta: bool) -> ModifiersState {
    let mut state = ModifiersState::empty();
    state.set(ModifiersState::SHIFT, shift);
    state.set(ModifiersState::CONTROL, control);
    state.set(ModifiersState::ALT, alt);
    state.set(ModifiersState::SUPER, meta);
    state
}

pub fn page_key(code: &str, name: &str, down: bool, mods: ModifiersState) -> Option<KeyInput> {
    let code = physical(code)?;
    let state = if down { ElementState::Pressed } else { ElementState::Released };
    let text = match name {
        "Enter" => Some("\r".to_owned()),
        _ if name.chars().count() == 1 && !mods.super_key() && !mods.control_key() => Some(name.to_owned()),
        _ => None,
    };
    page_keys::key(code, state, text.as_deref().filter(|_| down), mods)
}

fn physical(code: &str) -> Option<KeyCode> {
    use KeyCode as K;
    const LETTERS: [KeyCode; 26] = [
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
    ];
    const DIGITS: [KeyCode; 10] =
        [K::Digit0, K::Digit1, K::Digit2, K::Digit3, K::Digit4, K::Digit5, K::Digit6, K::Digit7, K::Digit8, K::Digit9];
    let index = |rest: &str, base: u8| rest.bytes().next().filter(|_| rest.len() == 1).map(|b| b.wrapping_sub(base));
    if let Some(i) = code.strip_prefix("Key").and_then(|rest| index(rest, b'A')) {
        return LETTERS.get(usize::from(i)).copied();
    }
    if let Some(i) = code.strip_prefix("Digit").and_then(|rest| index(rest, b'0')) {
        return DIGITS.get(usize::from(i)).copied();
    }
    Some(match code {
        "Enter" | "NumpadEnter" => K::Enter,
        "Backspace" => K::Backspace,
        "Tab" => K::Tab,
        "Escape" => K::Escape,
        "Space" => K::Space,
        "Delete" => K::Delete,
        "ArrowLeft" => K::ArrowLeft,
        "ArrowRight" => K::ArrowRight,
        "ArrowUp" => K::ArrowUp,
        "ArrowDown" => K::ArrowDown,
        "Home" => K::Home,
        "End" => K::End,
        "PageUp" => K::PageUp,
        "PageDown" => K::PageDown,
        "Minus" => K::Minus,
        "Equal" => K::Equal,
        "BracketLeft" => K::BracketLeft,
        "BracketRight" => K::BracketRight,
        "Backslash" => K::Backslash,
        "Semicolon" => K::Semicolon,
        "Quote" => K::Quote,
        "Comma" => K::Comma,
        "Period" => K::Period,
        "Slash" => K::Slash,
        "Backquote" => K::Backquote,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use superfik_client::input::keys::{KeyCommand, map_key};
    use superfik_client::state::EditKey;

    use super::*;

    fn command(name: &str, mods: ModifiersState) -> Option<KeyCommand> {
        key(name, mods).map(|(key, text)| map_key(&key, text.as_deref(), mods))
    }

    #[test]
    fn typed_characters_and_accents_insert_themselves() {
        let none = ModifiersState::empty();
        assert_eq!(command("é", none), Some(KeyCommand::Edit(EditKey::Insert("é".into()))));
        assert_eq!(command(" ", none), Some(KeyCommand::Edit(EditKey::Insert(" ".into()))));
        assert_eq!(
            command("A", modifiers(true, false, false, false)),
            Some(KeyCommand::Edit(EditKey::Insert("A".into())))
        );
    }

    #[test]
    fn named_keys_and_chords_are_commands() {
        let none = ModifiersState::empty();
        assert_eq!(command("Enter", none), Some(KeyCommand::Submit));
        assert_eq!(command("Backspace", none), Some(KeyCommand::Edit(EditKey::Backspace)));
        assert_eq!(command("v", modifiers(false, false, false, true)), Some(KeyCommand::Paste));
        assert_eq!(command("z", modifiers(false, true, false, false)), Some(KeyCommand::Undo));
    }

    #[test]
    fn page_keys_carry_chromiums_code_and_their_text_on_the_press() {
        let none = ModifiersState::empty();
        let a = page_key("KeyA", "a", true, none).expect("a");
        assert_eq!((a.code, a.text), (0x41, Some('a')));
        assert_eq!(page_key("KeyA", "a", false, none).and_then(|k| k.text), None, "released");
        assert_eq!(page_key("Enter", "Enter", true, none).and_then(|k| k.text), Some('\r'));
        assert_eq!(page_key("Digit7", "&", true, none).map(|k| k.code), Some(0x37));
        assert_eq!(page_key("Backspace", "Backspace", true, none).map(|k| (k.code, k.text)), Some((0x08, None)));
        assert_eq!(page_key("MetaLeft", "Meta", true, none), None);
        let command = modifiers(false, false, false, true);
        assert_eq!(page_key("KeyC", "c", true, command).and_then(|k| k.text), None, "⌘C types nothing");
    }

    #[test]
    fn modifiers_and_input_method_keys_type_nothing() {
        for name in ["Shift", "Meta", "Dead", "Process", "Unidentified", "CapsLock"] {
            assert_eq!(key(name, ModifiersState::empty()), None, "{name}");
        }
    }
}
