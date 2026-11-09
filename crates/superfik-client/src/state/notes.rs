//! The project's notes as the app knows them. The backend owns them:
//! they load with the workspace (`LoadNotes`), edits go out as
//! `EditNotes`, and every change (the user's or an agent's) comes back as
//! the whole book in `Notes`.
//!
//! A note belongs to the project; a session *links* it
//! (`Conversation::notes`). A note created from a session, by the user or
//! by the agent of one of its runs, starts linked to that session.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use superfik_core::note::{Note, NoteBrief, NoteEdit, NoteId, Paper};
use superfik_core::{ConversationId, RunId};
use superfik_protocol::ClientCommand;

use super::{Effect, Session};

#[derive(Debug, Clone, PartialEq)]
pub enum NoteAction {
    Link(NoteId),
    Unlink(NoteId),
    Toggle { id: NoteId, item: usize },
    Open(Option<NoteId>),
    Close,
    Pick(NoteId),
    Recolor(Paper),
}

impl NoteAction {
    pub(super) fn trigger(&self) -> super::sound::Trigger {
        use super::sound::Trigger;
        match self {
            NoteAction::Link(_) | NoteAction::Pick(_) => Trigger::Add,
            NoteAction::Unlink(_) => Trigger::Remove,
            NoteAction::Toggle { .. } | NoteAction::Recolor(_) => Trigger::ToggleReference,
            NoteAction::Open(_) | NoteAction::Close => Trigger::ToggleDraft,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NotesState {
    pub notes: Vec<Note>,
    pub loaded: bool,
    creating: VecDeque<ConversationId>,
    pub editing: Option<super::note_draft::NoteDraft>,
}

impl NotesState {
    pub fn get(&self, id: NoteId) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }
}

impl Session {
    pub(super) fn note_action(&mut self, action: NoteAction) -> Vec<Effect> {
        match action {
            NoteAction::Link(id) => self.link_note(id),
            NoteAction::Unlink(id) => self.unlink_note(id),
            NoteAction::Toggle { id, item } => self.toggle_note_item(id, item),
            NoteAction::Open(id) => self.open_note(id),
            NoteAction::Close => self.close_note(),
            NoteAction::Pick(id) => self.pick_note(id),
            NoteAction::Recolor(paper) => self.recolor_note(paper),
        }
    }

    fn pick_note(&mut self, id: NoteId) -> Vec<Effect> {
        let Some(slug) = self.notes.get(id).map(Note::slug) else { return Vec::new() };
        self.popover = None;
        self.conversation_mut().composer.complete_word(&format!("{}{slug}", super::connections::SKILL));
        Vec::new()
    }

    pub(super) fn link_named_notes(&mut self) {
        let named: Vec<NoteId> = self
            .composer_tokens()
            .iter()
            .filter_map(|t| match t.pick {
                super::Pick::Note(note) => Some(note.id),
                _ => None,
            })
            .collect();
        for id in named {
            self.link_note(id);
        }
    }

    pub(super) fn load_notes(path: PathBuf) -> Effect {
        Effect::Command(Box::new(ClientCommand::LoadNotes { path }))
    }

    pub(super) fn edit_notes(&self, edits: Vec<NoteEdit>) -> Vec<Effect> {
        let Some(path) = self.root.clone() else { return Vec::new() };
        vec![Effect::Command(Box::new(ClientCommand::EditNotes { path, edits }))]
    }

    pub(super) fn create_note(&mut self, body: String, paper: Paper) -> Vec<Effect> {
        if body.trim().is_empty() || self.root.is_none() {
            return Vec::new();
        }
        self.notes.creating.push_back(self.active);
        self.edit_notes(vec![NoteEdit::Create { body, paper }])
    }

    pub(super) fn link_note(&mut self, id: NoteId) -> Vec<Effect> {
        if self.notes.get(id).is_some() && !self.conversation().notes.contains(&id) {
            self.conversation_mut().notes.push(id);
        }
        Vec::new()
    }

    pub(super) fn unlink_note(&mut self, id: NoteId) -> Vec<Effect> {
        self.conversation_mut().notes.retain(|n| *n != id);
        Vec::new()
    }

    pub(super) fn linked_notes(&self) -> Vec<NoteBrief> {
        self.conversation().notes.iter().filter_map(|id| self.notes.get(*id)).map(Note::brief).collect()
    }

    pub(super) fn notes_arrived(&mut self, path: &Path, notes: Vec<Note>, by: Option<RunId>, created: &[NoteId]) {
        if self.root.as_deref() != Some(path) {
            return;
        }
        self.notes.notes = notes;
        self.notes.loaded = true;
        let owner = match by {
            Some(run) => self.conversations.iter().find(|c| c.owns_run(run)).map(|c| c.id),
            None if created.is_empty() => return,
            None => self.notes.creating.pop_front(),
        };
        let Some(conversation) = self.conversations.iter_mut().find(|c| Some(c.id) == owner) else { return };
        for id in created {
            if !conversation.notes.contains(id) {
                conversation.notes.push(*id);
            }
        }
    }
}
