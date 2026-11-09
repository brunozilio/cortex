use winit::keyboard::SmolStr;

use super::*;

fn character(c: &str) -> Key {
    Key::Character(SmolStr::new(c))
}

#[test]
fn typing_inserts_text() {
    let cmd = map_key(&character("h"), Some("h"), ModifiersState::empty());
    assert_eq!(cmd, KeyCommand::Edit(EditKey::Insert("h".into())));
}

#[test]
fn vertical_arrows_are_lines_not_the_text_edges() {
    let up = Key::Named(NamedKey::ArrowUp);
    assert_eq!(map_key(&up, None, ModifiersState::empty()), KeyCommand::Edit(EditKey::Up { select: false }));
    let down = Key::Named(NamedKey::ArrowDown);
    assert_eq!(map_key(&down, None, ModifiersState::SHIFT), KeyCommand::Edit(EditKey::Down { select: true }));
    assert_eq!(
        map_key(&Key::Named(NamedKey::End), None, ModifiersState::empty()),
        KeyCommand::Edit(EditKey::End { select: false })
    );
}

#[test]
fn enter_submits_and_shift_enter_breaks_line() {
    let enter = Key::Named(NamedKey::Enter);
    assert_eq!(map_key(&enter, Some("\r"), ModifiersState::empty()), KeyCommand::Submit);
    assert_eq!(map_key(&enter, Some("\r"), ModifiersState::SHIFT), KeyCommand::Edit(EditKey::Insert("\n".into())));
    assert_eq!(map_key(&enter, Some("\r"), ModifiersState::SUPER), KeyCommand::SubmitAlways);
}

#[test]
fn view_shortcuts() {
    let cmd = ModifiersState::SUPER;
    assert_eq!(map_key(&character("="), None, cmd), KeyCommand::ZoomIn);
    assert_eq!(map_key(&character("-"), None, cmd), KeyCommand::ZoomOut);
    assert_eq!(map_key(&character("0"), None, cmd), KeyCommand::ResetView);
}

#[test]
fn command_shortcuts() {
    assert_eq!(map_key(&character("v"), None, ModifiersState::SUPER), KeyCommand::Paste);
    assert_eq!(map_key(&character("a"), None, ModifiersState::SUPER), KeyCommand::Edit(EditKey::SelectAll));
    assert_eq!(map_key(&character("z"), None, ModifiersState::SUPER), KeyCommand::Undo);
    assert_eq!(map_key(&character("z"), None, ModifiersState::SUPER | ModifiersState::SHIFT), KeyCommand::Redo);
    let ctrl = ModifiersState::CONTROL;
    assert_eq!(map_key(&character("z"), None, ctrl), KeyCommand::Undo);
    assert_eq!(map_key(&character("Z"), None, ctrl | ModifiersState::SHIFT), KeyCommand::Redo);
    assert_eq!(map_key(&character("v"), None, ctrl), KeyCommand::Paste);
    assert_eq!(map_key(&character("a"), None, ctrl), KeyCommand::Edit(EditKey::SelectAll));
    assert_eq!(map_key(&character("n"), None, ctrl), KeyCommand::Ignore, "only the editing keys are taken");
    assert_eq!(map_key(&character("q"), None, ModifiersState::SUPER), KeyCommand::Ignore);
    assert_eq!(map_key(&character("n"), None, ModifiersState::SUPER), KeyCommand::NewConversation);
    assert_eq!(map_key(&character("1"), None, ModifiersState::SUPER), KeyCommand::SelectSession(1));
    assert_eq!(map_key(&character("9"), None, ModifiersState::SUPER), KeyCommand::SelectSession(9));
    assert_eq!(
        map_key(&character("1"), Some("1"), ModifiersState::empty()),
        KeyCommand::Edit(EditKey::Insert("1".into()))
    );
    assert_eq!(map_key(&character(","), None, ModifiersState::SUPER), KeyCommand::Settings);
    assert_eq!(map_key(&character("w"), None, ModifiersState::SUPER), KeyCommand::CloseWindow);
    assert_eq!(map_key(&character("t"), None, ModifiersState::SUPER), KeyCommand::OpenBrowser);
    assert_eq!(map_key(&character("l"), None, ModifiersState::SUPER), KeyCommand::OpenBrowser);
    assert_eq!(map_key(&character("["), None, ModifiersState::SUPER), KeyCommand::Back);
    let tab = Key::Named(NamedKey::Tab);
    let back = ModifiersState::CONTROL | ModifiersState::SHIFT;
    assert_eq!(map_key(&tab, Some("\t"), ModifiersState::CONTROL), KeyCommand::StepSession(Toward::Older));
    assert_eq!(map_key(&tab, Some("\t"), back), KeyCommand::StepSession(Toward::Newer));
    assert_eq!(map_key(&tab, Some("\t"), ModifiersState::empty()), KeyCommand::Tab { back: false });
    assert_eq!(map_key(&tab, Some("\t"), ModifiersState::SHIFT), KeyCommand::Tab { back: true });
}

fn windows(key: &Key, mods: ModifiersState) -> KeyCommand {
    map_key(key, None, for_primary(mods, Primary::Control))
}

#[test]
fn on_windows_ctrl_is_the_command_key() {
    let ctrl = ModifiersState::CONTROL;
    assert_eq!(windows(&character("n"), ctrl), KeyCommand::NewConversation);
    assert_eq!(windows(&character(","), ctrl), KeyCommand::Settings);
    assert_eq!(windows(&character("t"), ctrl), KeyCommand::OpenBrowser);
    assert_eq!(windows(&character("1"), ctrl), KeyCommand::SelectSession(1));
    assert_eq!(windows(&character("c"), ctrl), KeyCommand::Copy);
    assert_eq!(windows(&character("z"), ctrl | ModifiersState::SHIFT), KeyCommand::Redo);
    assert_eq!(windows(&Key::Named(NamedKey::Enter), ctrl), KeyCommand::SubmitAlways);
    let tab = Key::Named(NamedKey::Tab);
    assert_eq!(windows(&tab, ctrl), KeyCommand::StepSession(Toward::Older), "Ctrl+Tab still steps");
    let back = Key::Named(NamedKey::Backspace);
    assert_eq!(windows(&back, ctrl), KeyCommand::Edit(EditKey::BackspaceWord));
}

#[test]
fn on_windows_the_windows_key_is_never_the_apps() {
    let win = ModifiersState::SUPER;
    assert_eq!(windows(&character("n"), win), KeyCommand::Ignore);
    assert_eq!(windows(&Key::Named(NamedKey::Enter), win), KeyCommand::Submit);
    assert!(!for_primary(win, Primary::Control).super_key());
}

#[test]
fn on_macos_modifiers_are_left_as_they_are() {
    let mods = ModifiersState::SUPER | ModifiersState::CONTROL;
    assert_eq!(for_primary(mods, Primary::Command), mods);
    assert_eq!(
        map_key(&character("n"), None, for_primary(ModifiersState::CONTROL, Primary::Command)),
        KeyCommand::Ignore
    );
}

#[test]
fn in_a_browser_its_own_shortcuts_stay_its_own() {
    let web = Platform::WEB;
    let cmd = ModifiersState::SUPER;
    for key in ["n", "w", "t", "l", "r", "[", "]", "o", ",", "=", "-", "0", "1"] {
        assert_eq!(for_platform(map_key(&character(key), None, cmd), web), KeyCommand::Ignore, "⌘{key}");
        assert_ne!(for_platform(map_key(&character(key), None, cmd), Platform::DESKTOP), KeyCommand::Ignore);
    }
    for key in ["c", "v", "x", "z", "a", "\\"] {
        assert_ne!(for_platform(map_key(&character(key), None, cmd), web), KeyCommand::Ignore, "⌘{key} edits");
    }
    let enter = Key::Named(NamedKey::Enter);
    assert_eq!(for_platform(map_key(&enter, None, ModifiersState::empty()), web), KeyCommand::Submit);
}

#[test]
fn navigation_respects_shift_and_alt() {
    let left = Key::Named(NamedKey::ArrowLeft);
    assert_eq!(map_key(&left, None, ModifiersState::SHIFT), KeyCommand::Edit(EditKey::Left { select: true }));
    let back = Key::Named(NamedKey::Backspace);
    assert_eq!(map_key(&back, None, ModifiersState::ALT), KeyCommand::Edit(EditKey::BackspaceWord));
}

#[test]
fn control_characters_are_ignored() {
    assert_eq!(map_key(&character("\u{7f}"), Some("\u{7f}"), ModifiersState::empty()), KeyCommand::Ignore);
}
