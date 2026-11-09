//! The project's memory as the canvas shows it: the Memory node and the
//! explorer it expands into, with its search. The backend owns the
//! memories; edits go out as `EditMemory` and come back as `Memory`.
//! Searches go out as you type; what is lit stays until the backend's
//! hybrid answer (words and meaning) for the new text replaces it.

use std::path::PathBuf;

use superfik_core::{EngineEvent, Memory, MemoryEdit, MemoryId, MemoryKind, RunId};
use superfik_protocol::ClientCommand;
use superfik_ui::TextEdit;

use super::composer::apply_key;
use super::{EditKey, Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryField {
    Search,
    Draft,
    Revision,
}

#[derive(Debug, Clone)]
pub struct Revision {
    pub id: MemoryId,
    pub edit: TextEdit,
    pub kind: MemoryKind,
}

#[derive(Debug, Clone)]
pub struct MemoryState {
    pub memories: Vec<Memory>,
    pub loaded: bool,
    pub learned: Vec<MemoryId>,
    pub learned_at: f64,
    pub query: TextEdit,
    pub focus: Option<MemoryField>,
    pub open: bool,
    pub selected: Option<MemoryId>,
    pub draft: TextEdit,
    pub draft_kind: MemoryKind,
    pub revising: Option<Revision>,
    pub scroll: f32,
    pub activation: Vec<f32>,
    pub links: Vec<(usize, usize)>,
    pub lit: Vec<(MemoryId, f32)>,
    pub optimizing: Option<RunId>,
    pub can_undo: bool,
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            memories: Vec::new(),
            loaded: false,
            learned: Vec::new(),
            learned_at: f64::NEG_INFINITY,
            query: TextEdit::default(),
            focus: None,
            open: false,
            selected: None,
            draft: TextEdit::default(),
            draft_kind: MemoryKind::Semantic,
            revising: None,
            scroll: 0.0,
            activation: Vec::new(),
            links: Vec::new(),
            lit: Vec::new(),
            optimizing: None,
            can_undo: false,
        }
    }
}

impl MemoryState {
    pub fn searching(&self) -> bool {
        !self.query.text().trim().is_empty()
    }

    pub fn refresh(&mut self) {
        self.links = superfik_core::memory::links(&self.memories);
        self.activation = if self.searching() {
            let ids: Vec<MemoryId> = self.memories.iter().map(|m| m.id).collect();
            let previous = std::mem::take(&mut self.lit);
            self.lit = previous.into_iter().filter(|(id, _)| ids.contains(id)).collect();
            self.aligned()
        } else {
            self.lit.clear();
            vec![0.0; self.memories.len()]
        };
    }

    fn scored(&mut self, scores: &[(MemoryId, f32)]) {
        if !self.searching() {
            return;
        }
        self.lit = scores.to_vec();
        self.activation = self.aligned();
    }

    fn aligned(&self) -> Vec<f32> {
        self.memories.iter().map(|m| self.lit.iter().find(|(id, _)| *id == m.id).map_or(0.0, |(_, s)| *s)).collect()
    }

    pub fn listed(&self) -> Vec<usize> {
        if let Some(i) = self.selected.and_then(|id| self.memories.iter().position(|m| m.id == id)) {
            return vec![i];
        }
        let mut order: Vec<usize> = (0..self.memories.len()).rev().collect();
        if self.searching() {
            let scores = &self.activation;
            order.retain(|&i| scores[i] > 0.0);
            order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(b.cmp(&a)));
        }
        order
    }

    pub fn is_learned(&self, id: MemoryId) -> bool {
        self.learned.contains(&id)
    }
}

impl Session {
    pub(super) fn load_memory(path: PathBuf) -> Effect {
        Effect::Command(Box::new(ClientCommand::LoadMemory { path }))
    }

    pub(super) fn memory_arrived(
        &mut self,
        path: &std::path::Path,
        memories: Vec<Memory>,
        learned: Vec<MemoryId>,
        now: f64,
    ) -> Vec<Effect> {
        if self.root.as_deref() != Some(path) {
            return Vec::new();
        }
        self.memory.memories = memories;
        if self.memory.optimizing.is_some() {
            self.memory.can_undo = true;
        }
        self.memory.refresh();
        self.memory.loaded = true;
        if !learned.is_empty() {
            self.memory.learned = learned;
            self.memory.learned_at = now;
        }
        if self.memory.selected.is_some_and(|id| !self.memory.memories.iter().any(|m| m.id == id)) {
            self.memory.selected = None;
        }
        self.search_memory()
    }

    pub(super) fn memory_scored(&mut self, path: &std::path::Path, scores: &[(MemoryId, f32)]) {
        if self.root.as_deref() == Some(path) {
            self.memory.scored(scores);
        }
    }

    fn search_memory(&self) -> Vec<Effect> {
        let (Some(path), true) = (self.root.clone(), self.memory.searching()) else { return Vec::new() };
        let query = self.memory.query.text().to_owned();
        vec![Effect::Command(Box::new(ClientCommand::SearchMemory { path, query }))]
    }

    pub(super) fn edit_memory_field(&mut self, key: &EditKey) -> Option<Vec<Effect>> {
        match self.memory.focus? {
            MemoryField::Search => {
                let before = self.memory.query.text().to_owned();
                apply_key(&mut self.memory.query, key.clone());
                if self.memory.query.text() == before {
                    return Some(Vec::new());
                }
                self.memory.scroll = 0.0;
                self.memory.selected = None;
                self.memory.refresh();
                Some(self.search_memory())
            }
            MemoryField::Draft => {
                apply_key(&mut self.memory.draft, key.clone());
                Some(Vec::new())
            }
            MemoryField::Revision => {
                if let Some(revision) = &mut self.memory.revising {
                    apply_key(&mut revision.edit, key.clone());
                }
                Some(Vec::new())
            }
        }
    }

    pub(super) fn revise_memory(&mut self, id: MemoryId) -> Vec<Effect> {
        let Some(memory) = self.memory.memories.iter().find(|m| m.id == id) else { return Vec::new() };
        let mut edit = TextEdit::new(memory.text.clone());
        edit.select_all();
        self.memory.revising = Some(Revision { id, edit, kind: memory.kind });
        self.memory.selected = Some(id);
        self.memory.focus = Some(MemoryField::Revision);
        Vec::new()
    }

    pub(super) fn set_revision_kind(&mut self, kind: MemoryKind) -> Vec<Effect> {
        if let Some(revision) = &mut self.memory.revising {
            revision.kind = kind;
            self.memory.focus = Some(MemoryField::Revision);
        }
        Vec::new()
    }

    pub(super) fn save_revision(&mut self) -> Vec<Effect> {
        let Some(Revision { id, edit, kind }) = self.memory.revising.take() else { return Vec::new() };
        self.memory.focus = Some(MemoryField::Search);
        let text = edit.text().split_whitespace().collect::<Vec<_>>().join(" ");
        let Some(memory) = self.memory.memories.iter_mut().find(|m| m.id == id) else { return Vec::new() };
        let mut edits = Vec::new();
        if !text.is_empty() && text != memory.text {
            memory.text = text.clone();
            edits.push(MemoryEdit::Revise { id, text });
        }
        if kind != memory.kind {
            memory.kind = kind;
            edits.push(MemoryEdit::Reclassify { id, kind });
        }
        if edits.is_empty() { Vec::new() } else { self.edit_memory(edits) }
    }

    pub(super) fn focus_memory(&mut self, field: Option<MemoryField>) -> Vec<Effect> {
        self.memory.focus = field;
        if field.is_some() {
            self.popover = None;
            self.renaming = None;
        }
        Vec::new()
    }

    pub(super) fn open_memory(&mut self) -> Vec<Effect> {
        if self.root.is_some() {
            self.memory.open = true;
            self.popover = None;
            self.memory.focus = Some(MemoryField::Search);
        }
        Vec::new()
    }

    pub(super) fn close_memory(&mut self) -> Vec<Effect> {
        let memory = &mut self.memory;
        if matches!(memory.focus, Some(MemoryField::Draft | MemoryField::Revision)) {
            memory.revising = None;
            memory.focus = Some(MemoryField::Search);
            return Vec::new();
        }
        if memory.selected.take().is_some() {
            memory.scroll = 0.0;
            return Vec::new();
        }
        memory.open = false;
        memory.focus = None;
        memory.selected = None;
        memory.revising = None;
        memory.scroll = 0.0;
        memory.query.set("");
        memory.refresh();
        Vec::new()
    }

    pub(super) fn select_memory(&mut self, id: MemoryId) -> Vec<Effect> {
        self.memory.selected = if self.memory.selected == Some(id) { None } else { Some(id) };
        self.memory.scroll = 0.0;
        Vec::new()
    }

    pub(super) fn scroll_memory(&mut self, by: f32, max: f32) -> Vec<Effect> {
        self.memory.scroll = (self.memory.scroll + by).clamp(0.0, max.max(0.0));
        Vec::new()
    }

    pub(super) fn set_memory_kind(&mut self, kind: MemoryKind) -> Vec<Effect> {
        self.memory.draft_kind = kind;
        self.memory.focus = Some(MemoryField::Draft);
        Vec::new()
    }

    pub(super) fn remember(&mut self) -> Vec<Effect> {
        let text = self.memory.draft.text().trim().to_owned();
        if text.is_empty() {
            return Vec::new();
        }
        self.memory.draft.set("");
        let edit = MemoryEdit::Remember { kind: self.memory.draft_kind, text, tags: Vec::new() };
        self.edit_memory(vec![edit])
    }

    pub(super) fn forget_memory(&mut self, id: MemoryId) -> Vec<Effect> {
        self.memory.memories.retain(|m| m.id != id);
        self.memory.refresh();
        if self.memory.selected == Some(id) {
            self.memory.selected = None;
        }
        self.edit_memory(vec![MemoryEdit::Forget { id }])
    }

    fn edit_memory(&mut self, edits: Vec<MemoryEdit>) -> Vec<Effect> {
        let Some(path) = self.root.clone() else { return Vec::new() };
        self.memory.can_undo = false;
        vec![Effect::Command(Box::new(ClientCommand::EditMemory { path, edits }))]
    }

    pub(super) fn enter_memory(&mut self) -> Option<Vec<Effect>> {
        match self.memory.focus? {
            MemoryField::Draft => Some(self.remember()),
            MemoryField::Revision => Some(self.save_revision()),
            MemoryField::Search => {
                let best = self.memory.searching().then(|| self.memory.listed().first().copied()).flatten();
                if let Some(i) = best {
                    self.memory.selected = Some(self.memory.memories[i].id);
                    self.memory.scroll = 0.0;
                }
                Some(Vec::new())
            }
        }
    }

    pub(super) fn optimize_memory(&mut self) -> Vec<Effect> {
        let Some(path) = self.root.clone() else { return Vec::new() };
        if self.memory.optimizing.is_some() || self.memory.memories.len() < 2 {
            return Vec::new();
        }
        let run = RunId::new();
        self.memory.optimizing = Some(run);
        self.memory.can_undo = false;
        self.memory.revising = None;
        vec![Effect::Command(Box::new(ClientCommand::OptimizeMemory { run, path, model: self.model().clone() }))]
    }

    pub(super) fn undo_memory(&mut self) -> Vec<Effect> {
        let Some(path) = self.root.clone().filter(|_| self.memory.can_undo) else { return Vec::new() };
        self.memory.can_undo = false;
        vec![Effect::Command(Box::new(ClientCommand::UndoMemory { path }))]
    }

    pub(super) fn optimization_event(&mut self, run: RunId, event: &EngineEvent) -> bool {
        if self.memory.optimizing != Some(run) {
            return false;
        }
        if event.is_terminal() {
            self.memory.optimizing = None;
        }
        if let EngineEvent::Failed(message) = event {
            self.notice = Some(super::Notice::Raw(message.clone()));
        }
        true
    }
}
