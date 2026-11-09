//! Input forwarded to a page, in page pixels (physical, top-left origin).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub command: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointerButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyPhase {
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyInput {
    pub phase: KeyPhase,
    pub code: i32,
    pub text: Option<char>,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditCommand {
    Copy,
    Cut,
    Paste(String),
    SelectAll,
    Undo,
    Redo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PageInput {
    Move { x: i32, y: i32, modifiers: Modifiers },
    Press { x: i32, y: i32, button: PointerButton, modifiers: Modifiers },
    Release { x: i32, y: i32, button: PointerButton, modifiers: Modifiers },
    Wheel { x: i32, y: i32, dx: i32, dy: i32, modifiers: Modifiers },
    Leave,
    Key(KeyInput),
    Text(String),
    Edit(EditCommand),
}
