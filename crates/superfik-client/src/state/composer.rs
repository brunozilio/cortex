//! Composer interactions: typing, model and effort.

use superfik_protocol::ClientCommand;

use super::connections::SKILL;
use super::{EditKey, Effect, PopoverKind, Session};

pub(super) fn apply_key(edit: &mut superfik_ui::TextEdit, key: EditKey) {
    match key {
        EditKey::Insert(text) => edit.insert(&text),
        EditKey::Backspace => edit.backspace(),
        EditKey::BackspaceWord => edit.backspace_word(),
        EditKey::Delete => edit.delete(),
        EditKey::Left { select } => edit.move_left(select),
        EditKey::Right { select } => edit.move_right(select),
        EditKey::Home { select } => edit.move_home(select),
        EditKey::End { select } => edit.move_end(select),
        EditKey::Up { select } => edit.move_home(select),
        EditKey::Down { select } => edit.move_end(select),
        EditKey::MoveTo { offset, select } => edit.move_to(offset, select),
        EditKey::SelectWord(offset) => edit.select_word_at(offset),
        EditKey::SelectAll => edit.select_all(),
        EditKey::Undo => edit.undo(),
        EditKey::Redo => edit.redo(),
    }
}

impl Session {
    pub(super) fn edit(&mut self, key: EditKey) -> Vec<Effect> {
        if self.edit_pull_request(&key) || self.edit_review(&key) || self.edit_connect_sheet(&key) {
            return Vec::new();
        }
        if let Some(effects) =
            self.edit_memory_field(&key).or_else(|| self.edit_note(&key)).or_else(|| self.edit_address(&key))
        {
            return effects;
        }
        let renaming = self.renaming.is_some();
        let edit = match &mut self.renaming {
            Some((_, edit)) => edit,
            None => &mut self.conversation_mut().composer,
        };
        apply_key(edit, key);
        if !renaming {
            self.sync_skill_picker();
        }
        Vec::new()
    }

    fn sync_skill_picker(&mut self) {
        self.pick_cursor = self.exact_pick().unwrap_or(0);
        let typing = self.conversation().composer.trigger_query(&[SKILL]).is_some();
        match (typing, self.popover) {
            (true, None) => self.popover = Some(PopoverKind::Skills),
            (false, Some(PopoverKind::Skills)) => self.popover = None,
            _ => {}
        }
    }

    pub fn completion(&self) -> Option<String> {
        if self.renaming.is_some() {
            return None;
        }
        if let Some(suggestion) = self.suggestion() {
            return Some(suggestion.to_owned());
        }
        let composer = &self.conversation().composer;
        if composer.caret() != composer.text().len() {
            return None;
        }
        let (_, query) = composer.trigger_query(&[SKILL])?;
        if query.is_empty() {
            return None;
        }
        let typed = format!("{SKILL}{query}").to_ascii_lowercase();
        self.picks()
            .into_iter()
            .map(|pick| pick.label())
            .find(|label| label.to_ascii_lowercase().starts_with(&typed) && label.len() > typed.len())
            .and_then(|label| label.get(typed.len()..).map(str::to_owned))
    }

    pub(super) fn accept_completion(&mut self) -> Vec<Effect> {
        if self.suggestion().is_some() {
            return self.accept_suggestion();
        }
        let Some(rest) = self.completion() else { return Vec::new() };
        let composer = &mut self.conversation_mut().composer;
        composer.insert(&rest);
        composer.insert(" ");
        self.popover = None;
        Vec::new()
    }

    pub(super) fn complete_skill(&mut self, name: &str) -> Vec<Effect> {
        self.conversation_mut().composer.complete_word(&format!("{SKILL}{name}"));
        self.popover = None;
        Vec::new()
    }

    pub(super) fn clear_research(&mut self) -> Vec<Effect> {
        self.conversation_mut().research_items.clear();
        Vec::new()
    }

    pub(super) fn set_available_agents(&mut self, agents: Vec<superfik_core::CustomAgent>) {
        self.available_agents = agents;
    }

    pub(super) fn select_model(&mut self, index: usize, now: f64) -> Vec<Effect> {
        if self.is_model_selectable(index) && index != self.conversation().model {
            let previous = self.model().clone();
            self.model_changed = Some((previous.label, now));
            self.conversation_mut().model = index;
            self.conversation_mut().wanted_model = None;
            self.popover = None;
            let model = self.model();
            if (previous.engine, previous.provider, previous.account) != (model.engine, model.provider, model.account) {
                self.conversation_mut().forget_session();
            } else if !self.is_running() {
                return self.start_upkeep(self.active_index(), superfik_core::SessionCommand::MeasureContext);
            }
        }
        self.popover = None;
        Vec::new()
    }

    pub(super) fn toggle_popover(&mut self, kind: PopoverKind) -> Vec<Effect> {
        self.popover = if self.popover == Some(kind) { None } else { Some(kind) };
        let lists_github = matches!(kind, PopoverKind::Repository | PopoverKind::AttachRepository);
        if self.popover.is_some() && lists_github {
            return vec![Effect::Command(Box::new(ClientCommand::ListGitHubRepositories))];
        }
        if self.popover == Some(PopoverKind::PullRequests)
            && let Some(path) = self.workspace_root()
        {
            return vec![Effect::Command(Box::new(ClientCommand::ListPullRequests { path }))];
        }
        Vec::new()
    }

    pub(super) fn close_popover(&mut self) -> Vec<Effect> {
        self.popover = None;
        Vec::new()
    }
}
