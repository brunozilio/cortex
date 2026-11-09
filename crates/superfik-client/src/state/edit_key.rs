//! Editing keys forwarded to the composer.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKey {
    Insert(String),
    Backspace,
    BackspaceWord,
    Delete,
    Left { select: bool },
    Right { select: bool },
    Home { select: bool },
    End { select: bool },
    Up { select: bool },
    Down { select: bool },
    MoveTo { offset: usize, select: bool },
    SelectWord(usize),
    SelectAll,
    Undo,
    Redo,
}
