//! Sessions kept between launches. After every action the reducer
//! compares what would be kept of each conversation with what it was
//! before and asks the backend to keep the ones that changed; on opening
//! a workspace it asks for the kept ones and puts them back on the
//! timeline. Streaming text alone never triggers a save: a reply is kept
//! when it starts, when its status changes and when it ends.

use std::path::Path;

use superfik_core::{ConversationId, ModelOption, SessionRecord};
use superfik_protocol::ClientCommand;

use super::{Conversation, Effect, Session};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Kept {
    id: ConversationId,
    title: Option<String>,
    turns: usize,
    reply: Option<superfik_core::Activity>,
    engine_session: Option<superfik_core::SessionId>,
    model: ModelKey,
    effort: superfik_core::Effort,
    archived: bool,
    repositories: usize,
    pull_request: Option<u64>,
    references: usize,
    agents: usize,
    connections: usize,
    notes: Vec<superfik_core::note::NoteId>,
    visibility: superfik_core::sharing::Visibility,
}

impl Conversation {
    pub fn record(&self, models: &[ModelOption]) -> Option<SessionRecord> {
        let (model, model_account) = self.model_key(models);
        Some(SessionRecord {
            connections: self.connections.clone(),
            notes: self.notes.clone(),
            id: self.id,
            title: self.title.clone(),
            started_unix: self.started_unix?,
            turns: self.turns.clone(),
            engine_session: self.engine_session.clone(),
            model,
            model_account,
            effort: self.effort,
            archived: self.archived,
            repositories: self.repositories.clone(),
            pull_request: self.pull_request.clone(),
            references: self.references.clone(),
            agents: self.agents.clone(),
            research_items: self.research_items.clone(),
            visibility: self.visibility.clone(),
        })
    }

    pub fn from_record(record: SessionRecord, models: &[ModelOption]) -> Self {
        let record = record.settled();
        let wanted = (record.model.clone(), record.model_account);
        let found = find_model(models, &wanted);
        Self {
            wanted_model: found.is_none().then_some(wanted),
            id: record.id,
            title: record.title,
            started_unix: Some(record.started_unix),
            turns: record.turns,
            engine_session: record.engine_session,
            archived: record.archived,
            repositories: record.repositories,
            pull_request: record.pull_request,
            references: record.references,
            agents: record.agents,
            research_items: record.research_items,
            connections: record.connections,
            notes: record.notes,
            visibility: record.visibility,
            shared_by: None,
            carried: None,
            ..Self::new(found.unwrap_or(0), record.effort)
        }
    }

    pub fn model_key(&self, models: &[ModelOption]) -> ModelKey {
        if let Some(wanted) = &self.wanted_model {
            return wanted.clone();
        }
        models.get(self.model).or(models.last()).map(|m| (m.id.clone(), m.account)).unwrap_or_default()
    }

    fn kept(&self, models: &[ModelOption]) -> Option<Kept> {
        self.started_unix?;
        Some(Kept {
            id: self.id,
            title: self.title.clone(),
            turns: self.turns.len(),
            reply: self.turns.last().map(|t| t.response.activity.clone()),
            engine_session: self.engine_session.clone(),
            model: self.model_key(models),
            effort: self.effort,
            archived: self.archived,
            repositories: self.repositories.len(),
            pull_request: self.pull_request.as_ref().map(|pr| pr.number),
            references: self.references.len(),
            agents: self.agents.len(),
            connections: self.connections.len(),
            notes: self.notes.clone(),
            visibility: self.visibility.clone(),
        })
    }
}

pub type ModelKey = (String, Option<superfik_core::ConnectionId>);

pub fn find_model(models: &[ModelOption], key: &ModelKey) -> Option<usize> {
    let (id, account) = key;
    models.iter().position(|m| &m.id == id && &m.account == account).or_else(|| models.iter().position(|m| &m.id == id))
}

impl Session {
    pub(super) fn kept(&self) -> Vec<Kept> {
        self.conversations.iter().filter_map(|c| c.kept(&self.models)).collect()
    }

    pub(super) fn keep_changed(&self, before: &[Kept], loaded: bool) -> Vec<Effect> {
        let Some(path) = self.workspace_root() else { return Vec::new() };
        self.conversations
            .iter()
            .filter(|c| !(loaded && !before.iter().any(|k| k.id == c.id)))
            .filter(|c| c.shared_by.is_none())
            .filter_map(|c| Some((c, c.kept(&self.models)?)))
            .filter(|(_, now)| !before.contains(now))
            .filter_map(|(c, _)| c.record(&self.models))
            .map(|session| Effect::KeepSession {
                path: path.clone(),
                project: self.project(),
                session: Box::new(session),
            })
            .collect()
    }

    pub(super) fn load_sessions(path: std::path::PathBuf) -> Effect {
        Effect::Command(Box::new(ClientCommand::LoadSessions { path, project: None }))
    }

    pub(super) fn sessions_arrived(&mut self, path: &Path, sessions: Vec<SessionRecord>) {
        if self.workspace_root().as_deref() != Some(path) {
            return;
        }
        for record in sessions {
            if !self.conversations.iter().any(|c| c.id == record.id) {
                self.conversations.push(Conversation::from_record(record, &self.models));
            }
        }
    }
}

#[cfg(test)]
mod tests;
