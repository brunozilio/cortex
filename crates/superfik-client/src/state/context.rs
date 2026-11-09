//! Context attached to the next prompt: repositories, skills and references.

use std::path::PathBuf;

use superfik_core::{Repository, Skill};
use superfik_protocol::ClientCommand;

use super::{Conversation, Effect, Session};

pub fn skills_in(text: &str, available: &[Skill]) -> Vec<Skill> {
    let mut found: Vec<Skill> = Vec::new();
    for token in text.split_whitespace() {
        let Some(name) = token.strip_prefix('/') else { continue };
        if let Some(skill) = available.iter().find(|s| s.name == name)
            && !found.iter().any(|s| s.name == skill.name)
        {
            found.push(skill.clone());
        }
    }
    found
}

pub fn strip_skill(text: &str, name: &str) -> String {
    text.split_whitespace().filter(|token| token.strip_prefix('/') != Some(name)).collect::<Vec<_>>().join(" ")
}

const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

pub fn is_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

const MAX_RECENTS: usize = 8;

impl Session {
    pub(super) fn add_repository(&mut self, repository: Repository) -> Vec<Effect> {
        self.popover = None;
        if self.workspace_root().as_deref() == Some(repository.root.as_path()) {
            return Vec::new();
        }
        let repositories = &mut self.conversation_mut().repositories;
        if !repositories.iter().any(|r| r.root == repository.root) {
            repositories.push(repository);
        }
        Vec::new()
    }

    pub(super) fn remove_repository(&mut self, index: usize) -> Vec<Effect> {
        let repositories = &mut self.conversation_mut().repositories;
        if index < repositories.len() {
            repositories.remove(index);
        }
        Vec::new()
    }

    pub fn addable_skills(&self) -> Vec<&Skill> {
        let selected = self.selected_skills();
        self.available_skills.iter().filter(|s| !selected.contains(s)).collect()
    }

    pub(super) fn remove_skill(&mut self, name: &str) -> Vec<Effect> {
        let c = self.conversation_mut();
        if c.composer.text().split_whitespace().any(|token| token.strip_prefix('/') == Some(name)) {
            let text = strip_skill(c.composer.text(), name);
            c.composer.set(text);
        }
        if !c.removed_skills.iter().any(|n| n == name) {
            c.removed_skills.push(name.to_owned());
        }
        self.flip(super::Carousel::Skills, 0)
    }

    pub(super) fn add_skill(&mut self, name: &str) -> Vec<Effect> {
        self.popover = None;
        self.conversation_mut().removed_skills.retain(|n| n != name);
        Vec::new()
    }

    pub(super) fn add_references(&mut self, paths: Vec<PathBuf>) -> Vec<Effect> {
        if self.remote {
            let conversation = self.conversation_mut();
            let fresh: Vec<PathBuf> = paths
                .into_iter()
                .filter(|path| {
                    !conversation.uploading.contains(path) && !conversation.previews.values().any(|p| p == path)
                })
                .collect();
            conversation.uploading.extend(fresh.iter().cloned());
            return if fresh.is_empty() { Vec::new() } else { vec![Effect::Upload(fresh)] };
        }
        let references = &mut self.conversation_mut().references;
        for path in paths {
            if !references.contains(&path) {
                references.push(path);
            }
        }
        Vec::new()
    }

    pub(super) fn uploaded(&mut self, from: PathBuf, path: PathBuf) -> Vec<Effect> {
        let Some(conversation) = self.conversations.iter_mut().find(|c| c.uploading.contains(&from)) else {
            return Vec::new();
        };
        conversation.uploading.retain(|p| p != &from);
        conversation.previews.insert(path.clone(), from);
        conversation.references.push(path);
        Vec::new()
    }

    pub(super) fn remove_references(&mut self) -> Vec<Effect> {
        let c = self.conversation_mut();
        c.references.clear();
        c.expanded_reference = None;
        Vec::new()
    }

    pub(super) fn toggle_reference(&mut self, index: usize) -> Vec<Effect> {
        let c = self.conversation_mut();
        c.expanded_reference = match c.expanded_reference {
            Some(open) if open == index => None,
            _ if index < c.references.len() => Some(index),
            _ => None,
        };
        Vec::new()
    }

    pub(super) fn remove_reference(&mut self, index: usize) -> Vec<Effect> {
        let c = self.conversation_mut();
        if index >= c.references.len() {
            return Vec::new();
        }
        c.references.remove(index);
        c.expanded_reference = match c.expanded_reference {
            Some(open) if open == index => None,
            Some(open) if open > index => Some(open - 1),
            other => other,
        };
        Vec::new()
    }

    pub(super) fn open_workspace(&mut self, path: PathBuf) -> Vec<Effect> {
        let mut effects = self.cancel_all();
        let mut recents = std::mem::take(&mut self.recent_workspaces);
        recents.retain(|p| p != &path);
        recents.insert(0, path.clone());
        recents.truncate(MAX_RECENTS);
        self.reset_workspace(Some(path.clone()), recents);
        effects.extend([
            Effect::Command(Box::new(ClientCommand::InspectWorkspace { path: path.clone() })),
            Effect::Command(Box::new(ClientCommand::ListSkills { path: path.clone() })),
            Effect::Command(Box::new(ClientCommand::ListAgents { path: path.clone() })),
            Session::load_memory(path.clone()),
            Session::load_notes(path.clone()),
            Session::load_routines(path.clone()),
            Session::load_sessions(path.clone()),
            Effect::Persist,
        ]);
        effects
    }

    pub(super) fn reset_workspace(&mut self, root: Option<PathBuf>, recents: Vec<PathBuf>) {
        let conversation = Conversation::new(self.conversation().model, self.conversation().effort);
        *self = Session {
            platform: self.platform,
            recent_workspaces: recents,
            models: std::mem::take(&mut self.models),
            engines: std::mem::take(&mut self.engines),
            github: self.github.take(),
            clone_root: std::mem::take(&mut self.clone_root),
            preferences: self.preferences,
            get_started: self.get_started,
            onboarded: self.onboarded,
            clock: self.clock,
            remote: self.remote,
            organization: std::mem::take(&mut self.organization),
            server_link: std::mem::take(&mut self.server_link),
            lock: std::mem::take(&mut self.lock),
            managed: std::mem::take(&mut self.managed),
            active: conversation.id,
            conversations: vec![conversation],
            ..Session::new(root)
        };
    }

    pub(super) fn remove_workspace(&mut self) -> Vec<Effect> {
        if self.root.is_none() {
            return Vec::new();
        }
        let mut attached = std::mem::take(&mut self.conversation_mut().repositories);
        if attached.is_empty() {
            let mut effects = self.cancel_all();
            let recents = std::mem::take(&mut self.recent_workspaces);
            self.reset_workspace(None, recents);
            effects.push(Effect::Persist);
            return effects;
        }
        let next = attached.remove(0);
        let effects = self.open_workspace(next.root);
        self.conversation_mut().repositories = attached;
        effects
    }
}
