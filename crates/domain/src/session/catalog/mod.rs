use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::SessionId;
use crate::conversation::SessionStatus;

pub const KEPT: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listed {
    pub session: SessionId,
    pub cwd: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub created: u64,
    pub updated: u64,
    pub status: SessionStatus,
    #[serde(default)]
    pub unread: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Catalog {
    rows: Vec<Listed>,
}

impl Catalog {
    pub fn listed(&self) -> &[Listed] {
        &self.rows
    }

    pub fn find(&self, session: &SessionId) -> Option<&Listed> {
        self.rows.iter().find(|row| row.session == *session)
    }

    pub fn started(&mut self, session: &SessionId, cwd: &Path, now: u64) {
        match self.row(session) {
            Some(row) => {
                row.cwd = cwd.to_path_buf();
                row.updated = now;
                row.status = SessionStatus::Running;
                row.unread = false;
            }
            None => self.rows.push(Listed {
                session: session.clone(),
                cwd: cwd.to_path_buf(),
                title: None,
                created: now,
                updated: now,
                status: SessionStatus::Running,
                unread: false,
            }),
        }
        self.order();
    }

    pub fn status(&mut self, session: &SessionId, status: SessionStatus, now: u64) {
        if let Some(row) = self.row(session) {
            row.status = status;
            row.updated = now;
            row.unread = !matches!(status, SessionStatus::Running | SessionStatus::Thinking);
        }
        self.order();
    }

    pub fn seen(&mut self, session: &SessionId) {
        if let Some(row) = self.row(session) {
            row.unread = false;
        }
    }

    pub fn rename(&mut self, session: &SessionId, title: &str) {
        if let Some(row) = self.row(session) {
            row.title = Some(title.to_owned());
        }
    }

    pub fn delete(&mut self, session: &SessionId) {
        self.rows.retain(|row| row.session != *session);
    }

    pub fn import(&mut self, owner_settings: &serde_json::Value) {
        let Some(folders) = owner_settings["conversations"].as_object() else { return };
        for (cwd, kept) in folders {
            for conversation in kept.as_array().into_iter().flatten() {
                let Some(session) = conversation["session"].as_str().map(|id| SessionId(id.to_owned())) else {
                    continue;
                };
                if self.find(&session).is_some() {
                    continue;
                }
                let at = conversation["at"].as_u64().unwrap_or(0);
                self.rows.push(Listed {
                    session,
                    cwd: PathBuf::from(cwd),
                    title: conversation["title"].as_str().map(str::to_owned),
                    created: at,
                    updated: at,
                    status: SessionStatus::Done,
                    unread: false,
                });
            }
        }
        self.order();
    }

    fn row(&mut self, session: &SessionId) -> Option<&mut Listed> {
        self.rows.iter_mut().find(|row| row.session == *session)
    }

    fn order(&mut self) {
        self.rows.sort_by_key(|row| std::cmp::Reverse(row.updated));
        self.rows.truncate(KEPT);
    }
}

#[cfg(test)]
mod tests;
