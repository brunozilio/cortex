//! Keyboard mapping: winit keys to editing commands. Pure and tested.

use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::state::{EditKey, Platform, Primary, Toward};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCommand {
    Edit(EditKey),
    Submit,
    SubmitAlways,
    Escape,
    Paste,
    Copy,
    Cut,
    Undo,
    Redo,
    OpenWorkspace,
    NewConversation,
    SelectSession(usize),
    StepSession(Toward),
    Tab { back: bool },
    Settings,
    CloseWindow,
    OpenBrowser,
    Reload,
    Back,
    Forward,
    FillLogin,
    ZoomIn,
    ZoomOut,
    ResetView,
    Ignore,
}

impl KeyCommand {
    pub fn kept_by_browser(&self) -> bool {
        matches!(
            self,
            Self::NewConversation
                | Self::CloseWindow
                | Self::OpenBrowser
                | Self::Reload
                | Self::Back
                | Self::Forward
                | Self::OpenWorkspace
                | Self::Settings
                | Self::SelectSession(_)
                | Self::ZoomIn
                | Self::ZoomOut
                | Self::ResetView
        )
    }
}

pub fn for_primary(mods: ModifiersState, primary: Primary) -> ModifiersState {
    match primary {
        Primary::Command => mods,
        Primary::Control => {
            let mut mods = mods - ModifiersState::SUPER;
            if mods.control_key() {
                mods |= ModifiersState::SUPER;
            }
            mods
        }
    }
}

pub fn for_platform(command: KeyCommand, platform: Platform) -> KeyCommand {
    if !platform.browser_shortcuts && command.kept_by_browser() { KeyCommand::Ignore } else { command }
}

pub fn map_key(key: &Key, text: Option<&str>, mods: ModifiersState) -> KeyCommand {
    let select = mods.shift_key();
    let command = mods.super_key();
    if let Key::Named(named) = key {
        return match named {
            NamedKey::Tab if mods.control_key() => {
                KeyCommand::StepSession(if select { Toward::Newer } else { Toward::Older })
            }
            NamedKey::Tab if !command => KeyCommand::Tab { back: select },
            NamedKey::Enter if select => KeyCommand::Edit(EditKey::Insert("\n".into())),
            NamedKey::Enter if command => KeyCommand::SubmitAlways,
            NamedKey::Enter => KeyCommand::Submit,
            NamedKey::Escape => KeyCommand::Escape,
            NamedKey::Backspace if mods.alt_key() || (command && mods.control_key()) => {
                KeyCommand::Edit(EditKey::BackspaceWord)
            }
            NamedKey::Backspace => KeyCommand::Edit(EditKey::Backspace),
            NamedKey::Delete => KeyCommand::Edit(EditKey::Delete),
            NamedKey::ArrowLeft if command => KeyCommand::Edit(EditKey::Home { select }),
            NamedKey::ArrowRight if command => KeyCommand::Edit(EditKey::End { select }),
            NamedKey::ArrowLeft => KeyCommand::Edit(EditKey::Left { select }),
            NamedKey::ArrowRight => KeyCommand::Edit(EditKey::Right { select }),
            NamedKey::ArrowUp => KeyCommand::Edit(EditKey::Up { select }),
            NamedKey::ArrowDown => KeyCommand::Edit(EditKey::Down { select }),
            NamedKey::Home => KeyCommand::Edit(EditKey::Home { select }),
            NamedKey::End => KeyCommand::Edit(EditKey::End { select }),
            NamedKey::Space if !command => KeyCommand::Edit(EditKey::Insert(" ".into())),
            _ => KeyCommand::Ignore,
        };
    }
    if mods.control_key()
        && !command
        && let Some(editing) = match key.to_text() {
            Some("a") => Some(KeyCommand::Edit(EditKey::SelectAll)),
            Some("c") => Some(KeyCommand::Copy),
            Some("v") => Some(KeyCommand::Paste),
            Some("x") => Some(KeyCommand::Cut),
            Some("z" | "Z") => Some(if select { KeyCommand::Redo } else { KeyCommand::Undo }),
            _ => None,
        }
    {
        return editing;
    }
    if command {
        return match key.to_text() {
            Some("a") => KeyCommand::Edit(EditKey::SelectAll),
            Some("z" | "Z") if select => KeyCommand::Redo,
            Some("z") => KeyCommand::Undo,
            Some("v") => KeyCommand::Paste,
            Some("c") => KeyCommand::Copy,
            Some("x") => KeyCommand::Cut,
            Some("o") => KeyCommand::OpenWorkspace,
            Some("n") => KeyCommand::NewConversation,
            Some(digit @ ("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")) => {
                KeyCommand::SelectSession(digit.parse().unwrap_or(1))
            }
            Some(",") => KeyCommand::Settings,
            Some("w") => KeyCommand::CloseWindow,
            Some("t" | "l") => KeyCommand::OpenBrowser,
            Some("r") => KeyCommand::Reload,
            Some("[") => KeyCommand::Back,
            Some("]") => KeyCommand::Forward,
            Some("\\") => KeyCommand::FillLogin,
            Some("=" | "+") => KeyCommand::ZoomIn,
            Some("-") => KeyCommand::ZoomOut,
            Some("0") => KeyCommand::ResetView,
            _ => KeyCommand::Ignore,
        };
    }
    match text.filter(|t| !t.chars().any(char::is_control)) {
        Some(text) if !text.is_empty() => KeyCommand::Edit(EditKey::Insert(text.to_owned())),
        _ => KeyCommand::Ignore,
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
