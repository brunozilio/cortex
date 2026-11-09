//! A note being written on the Notes node: a new one or an existing one
//! opened in place. While a draft is open it takes the keys (Enter adds a
//! line); closing it (Escape, a click elsewhere, opening another) saves:
//! a new note is created linked to the session, an existing one is revised
//! against the revision it was opened at, and an emptied one is deleted.

use superfik_core::note::{Line, NoteEdit, NoteId, Paper};
use superfik_ui::TextEdit;

use super::composer::apply_key;
use super::{EditKey, Effect, Session};

#[derive(Debug, Clone)]
pub struct NoteDraft {
    pub id: Option<NoteId>,
    pub edit: TextEdit,
    pub base: u32,
    pub paper: Paper,
}

impl Session {
    pub(super) fn open_note(&mut self, id: Option<NoteId>) -> Vec<Effect> {
        if self.notes.editing.as_ref().is_some_and(|d| d.id.is_some() && d.id == id) {
            return Vec::new();
        }
        let effects = self.close_note();
        let (text, base, paper) = match id.and_then(|id| self.notes.get(id)) {
            Some(note) => (note.body.clone(), note.revision, note.paper),
            None if id.is_some() => return effects,
            None => (String::new(), 0, Paper::default()),
        };
        let mut edit = TextEdit::default();
        edit.set(&text);
        self.notes.editing = Some(NoteDraft { id, edit, base, paper });
        effects
    }

    pub(super) fn recolor_note(&mut self, paper: Paper) -> Vec<Effect> {
        if let Some(draft) = self.notes.editing.as_mut() {
            draft.paper = paper;
        }
        Vec::new()
    }

    pub(super) fn close_note(&mut self) -> Vec<Effect> {
        let Some(draft) = self.notes.editing.take() else { return Vec::new() };
        let body = draft.edit.text().to_owned();
        let paper = draft.paper;
        let Some(id) = draft.id else { return self.create_note(body, paper) };
        let Some(note) = self.notes.get(id) else { return Vec::new() };
        if body.trim().is_empty() {
            self.unlink_note(id);
            return self.edit_notes(vec![NoteEdit::Delete { id }]);
        }
        let mut edits = Vec::new();
        if note.body != body {
            edits.push(NoteEdit::Revise { id, body, base: Some(draft.base) });
        }
        if note.paper != paper {
            edits.push(NoteEdit::Recolor { id, paper });
        }
        if edits.is_empty() { Vec::new() } else { self.edit_notes(edits) }
    }

    pub(super) fn edit_note(&mut self, key: &EditKey) -> Option<Vec<Effect>> {
        let draft = self.notes.editing.as_mut()?;
        apply_key(&mut draft.edit, key.clone());
        Some(Vec::new())
    }

    pub(super) fn enter_note(&mut self) -> Option<Vec<Effect>> {
        let draft = self.notes.editing.as_mut()?;
        let before = &draft.edit.text()[..draft.edit.caret()];
        let line = before.rsplit('\n').next().unwrap_or("");
        let carry = match superfik_core::note::parse_line(line) {
            Line::Check { .. } => "\n- [ ] ",
            Line::Text(_) => "\n",
        };
        draft.edit.insert(carry);
        Some(Vec::new())
    }

    pub(super) fn toggle_note_item(&mut self, id: NoteId, item: usize) -> Vec<Effect> {
        let Some(note) = self.notes.get(id) else { return Vec::new() };
        let done = note
            .lines()
            .iter()
            .filter_map(|l| if let Line::Check { done, .. } = l { Some(*done) } else { None })
            .nth(item);
        match done {
            Some(done) => self.edit_notes(vec![NoteEdit::Check { id, item, done: !done }]),
            None => Vec::new(),
        }
    }
}
