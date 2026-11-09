//! Undo and redo for a single-field edit. A run of typing, or of deleting,
//! is one step; moving the caret, selecting or another kind of edit starts
//! a new one. Any new edit after an undo forgets the redo steps.

use super::TextEdit;

const LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Typing,
    Deleting,
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    run: Option<Kind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: Option<usize>,
}

impl TextEdit {
    pub(super) fn checkpoint(&mut self, kind: Kind) {
        if self.history.run != Some(kind) || kind == Kind::Other {
            let before = self.snapshot();
            self.history.undo.push(before);
            if self.history.undo.len() > LIMIT {
                self.history.undo.remove(0);
            }
            self.history.redo.clear();
        }
        self.history.run = Some(kind);
    }

    pub(super) fn end_run(&mut self) {
        self.history.run = None;
    }

    pub fn undo(&mut self) {
        let current = self.snapshot();
        if let Some(previous) = self.pop_changed(Direction::Undo) {
            self.history.redo.push(current);
            self.restore(previous);
        }
    }

    pub fn redo(&mut self) {
        let current = self.snapshot();
        if let Some(next) = self.pop_changed(Direction::Redo) {
            self.history.undo.push(current);
            self.restore(next);
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { text: self.text.clone(), caret: self.caret, anchor: self.anchor }
    }

    fn restore(&mut self, state: Snapshot) {
        self.text = state.text;
        self.caret = state.caret;
        self.anchor = state.anchor;
        self.history.run = None;
    }

    fn pop_changed(&mut self, direction: Direction) -> Option<Snapshot> {
        self.end_run();
        let stack = match direction {
            Direction::Undo => &mut self.history.undo,
            Direction::Redo => &mut self.history.redo,
        };
        while let Some(state) = stack.pop() {
            if state.text != self.text {
                return Some(state);
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Undo,
    Redo,
}

#[cfg(test)]
mod tests {
    use super::super::TextEdit;

    #[test]
    fn a_run_of_typing_is_one_step() {
        let mut edit = TextEdit::default();
        for c in "hello".chars() {
            edit.insert(&c.to_string());
        }
        edit.undo();
        assert_eq!(edit.text(), "");
        edit.redo();
        assert_eq!(edit.text(), "hello");
    }

    #[test]
    fn moving_the_caret_starts_a_new_step() {
        let mut edit = TextEdit::default();
        edit.insert("ab");
        edit.move_left(false);
        edit.insert("X");
        edit.undo();
        assert_eq!(edit.text(), "ab");
        edit.undo();
        assert_eq!(edit.text(), "");
    }

    #[test]
    fn undo_restores_the_selection_it_replaced() {
        let mut edit = TextEdit::new("hello world");
        edit.select_all();
        edit.insert("bye");
        edit.undo();
        assert_eq!((edit.text(), edit.selection()), ("hello world", Some((0, 11))));
    }

    #[test]
    fn a_new_edit_after_an_undo_forgets_the_redo_steps() {
        let mut edit = TextEdit::default();
        edit.insert("one");
        edit.undo();
        edit.insert("two");
        edit.redo();
        assert_eq!(edit.text(), "two");
    }

    #[test]
    fn undo_with_nothing_to_undo_does_nothing() {
        let mut edit = TextEdit::new("kept");
        edit.undo();
        edit.redo();
        assert_eq!(edit.text(), "kept");
    }

    #[test]
    fn a_run_of_deleting_is_undone_at_once() {
        let mut edit = TextEdit::new("abc");
        edit.backspace();
        edit.backspace();
        edit.undo();
        assert_eq!(edit.text(), "abc");
        edit.redo();
        assert_eq!(edit.text(), "a");
    }
}
