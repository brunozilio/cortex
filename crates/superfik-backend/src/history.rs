//! Where sessions are kept between launches. A [`SessionStore`] keeps
//! each owner's sessions per project; [`Sessions`] answers the client.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use superfik_core::{ConversationId, Principal, SessionRecord};
use superfik_protocol::ServerEvent;

use crate::memory::{app_data_dir, owner_key, project_key};

pub trait SessionStore: Send + Sync {
    fn load(&self, owner: &Principal, project: &Path) -> Vec<SessionRecord>;
    fn save(&self, owner: &Principal, project: &Path, session: &SessionRecord) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct FileSessionStore {
    root: PathBuf,
}

impl FileSessionStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("sessions"))
    }

    fn dir(&self, owner: &Principal, project: &Path) -> PathBuf {
        self.root.join(owner_key(owner)).join(project_key(project))
    }
}

impl SessionStore for FileSessionStore {
    fn load(&self, owner: &Principal, project: &Path) -> Vec<SessionRecord> {
        let Ok(entries) = std::fs::read_dir(self.dir(owner, project)) else { return Vec::new() };
        let mut sessions: Vec<SessionRecord> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .filter_map(|path| {
                let parsed = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| serde_json::from_slice::<SessionRecord>(&bytes).map_err(|e| e.to_string()));
                parsed.map_err(|error| tracing::warn!(%error, path = %path.display(), "skipping a kept session")).ok()
            })
            .collect();
        sessions.sort_by_key(|s| s.started_unix);
        sessions
    }

    fn save(&self, owner: &Principal, project: &Path, session: &SessionRecord) -> std::io::Result<()> {
        let dir = self.dir(owner, project);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_vec_pretty(session).map_err(std::io::Error::other)?;
        let path = dir.join(format!("{}.json", session.id));
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json)?;
        std::fs::rename(partial, path)
    }
}

#[derive(Debug, Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<HashMap<(String, PathBuf), HashMap<ConversationId, SessionRecord>>>,
}

impl SessionStore for InMemorySessionStore {
    fn load(&self, owner: &Principal, project: &Path) -> Vec<SessionRecord> {
        let sessions = self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut kept: Vec<SessionRecord> = sessions
            .get(&(owner_key(owner), project.to_path_buf()))
            .map(|by_id| by_id.values().cloned().collect())
            .unwrap_or_default();
        kept.sort_by_key(|s| s.started_unix);
        kept
    }

    fn save(&self, owner: &Principal, project: &Path, session: &SessionRecord) -> std::io::Result<()> {
        let mut sessions = self.sessions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        sessions.entry((owner_key(owner), project.to_path_buf())).or_default().insert(session.id, session.clone());
        Ok(())
    }
}

#[derive(Clone)]
pub struct Sessions {
    store: Arc<dyn SessionStore>,
}

impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sessions").finish_non_exhaustive()
    }
}

impl Sessions {
    pub fn new(store: Arc<dyn SessionStore>) -> Self {
        Self { store }
    }

    pub fn load(&self, owner: &Principal, project: &Path) -> ServerEvent {
        ServerEvent::Sessions { path: project.to_path_buf(), sessions: self.store.load(owner, project) }
    }

    pub fn save(&self, owner: &Principal, project: &Path, session: &SessionRecord) -> Option<ServerEvent> {
        let error = self.store.save(owner, project, session).err()?;
        tracing::warn!(%error, "could not keep a session");
        Some(ServerEvent::error(format!("Could not save the session: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Effort, OrgId, UserId};

    use super::*;

    fn record(started_unix: i64, title: &str) -> SessionRecord {
        SessionRecord {
            connections: Vec::new(),
            id: ConversationId::new(),
            title: Some(title.into()),
            started_unix,
            turns: Vec::new(),
            engine_session: None,
            model: "claude-opus-5-5".into(),
            model_account: None,
            effort: Effort::Medium,
            archived: false,
            repositories: Vec::new(),
            pull_request: None,
            references: Vec::new(),
            agents: Vec::new(),
            research_items: Vec::new(),
        }
    }

    fn stores(dir: &Path) -> [Box<dyn SessionStore>; 2] {
        [Box::new(FileSessionStore::new(dir.to_path_buf())), Box::new(InMemorySessionStore::default())]
    }

    #[test]
    fn saved_sessions_load_oldest_first_and_saving_again_replaces() {
        let dir = tempfile::tempdir().expect("tempdir");
        for store in stores(dir.path()) {
            let owner = Principal::LocalOwner;
            let (newer, mut older) = (record(20, "b"), record(10, "a"));
            store.save(&owner, Path::new("/r"), &newer).expect("save");
            store.save(&owner, Path::new("/r"), &older).expect("save");
            older.title = Some("renamed".into());
            store.save(&owner, Path::new("/r"), &older).expect("save");
            assert_eq!(store.load(&owner, Path::new("/r")), vec![older.clone(), newer.clone()]);
        }
    }

    #[test]
    fn sessions_are_kept_apart_by_owner_and_project() {
        let dir = tempfile::tempdir().expect("tempdir");
        for store in stores(dir.path()) {
            store.save(&Principal::LocalOwner, Path::new("/r"), &record(1, "a")).expect("save");
            assert!(store.load(&Principal::LocalOwner, Path::new("/other")).is_empty());
            assert!(store.load(&Principal::member(UserId::new(), OrgId::new()), Path::new("/r")).is_empty());
        }
    }

    #[test]
    fn unreadable_files_are_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionStore::new(dir.path().to_path_buf());
        let kept = record(1, "a");
        store.save(&Principal::LocalOwner, Path::new("/r"), &kept).expect("save");
        let folder = store.dir(&Principal::LocalOwner, Path::new("/r"));
        std::fs::write(folder.join("broken.json"), "{nope").expect("write");
        std::fs::write(folder.join("x.json.partial"), "{}").expect("write");
        assert_eq!(store.load(&Principal::LocalOwner, Path::new("/r")), vec![kept]);
    }

    #[test]
    fn loading_answers_with_the_project_path() {
        let sessions = Sessions::new(Arc::new(InMemorySessionStore::default()));
        let kept = record(1, "a");
        assert!(sessions.save(&Principal::LocalOwner, Path::new("/r"), &kept).is_none());
        assert_eq!(
            sessions.load(&Principal::LocalOwner, Path::new("/r")),
            ServerEvent::Sessions { path: "/r".into(), sessions: vec![kept] }
        );
    }
}
