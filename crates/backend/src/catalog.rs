use std::path::PathBuf;
use std::sync::Mutex;

use domain::Principal;
use domain::catalog::Catalog;
use platform::sync::Locked;

use crate::dirs::{app_data_dir, owner_key};

pub trait SessionCatalog: Send + Sync {
    fn read(&self, owner: &Principal) -> Catalog;
    fn update(&self, owner: &Principal, change: &mut dyn FnMut(&mut Catalog));
}

#[derive(Debug)]
pub struct FileSessionCatalog {
    root: PathBuf,
    writing: Mutex<()>,
}

impl FileSessionCatalog {
    pub fn new(root: PathBuf) -> Self {
        Self { root, writing: Mutex::new(()) }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("users"))
    }

    fn path(&self, owner: &Principal) -> PathBuf {
        self.root.join(owner_key(owner)).join("sessions.json")
    }

    fn load(&self, owner: &Principal) -> Catalog {
        std::fs::read(self.path(owner)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }
}

impl SessionCatalog for FileSessionCatalog {
    fn read(&self, owner: &Principal) -> Catalog {
        let _writing = self.writing.locked();
        self.load(owner)
    }

    fn update(&self, owner: &Principal, change: &mut dyn FnMut(&mut Catalog)) {
        let _writing = self.writing.locked();
        let before = self.load(owner);
        let mut catalog = before.clone();
        change(&mut catalog);
        if catalog == before {
            return;
        }
        let kept = serde_json::to_vec(&catalog)
            .map_err(|error| error.to_string())
            .and_then(|json| platform::private_file::replace(&self.path(owner), &json).map_err(|e| e.to_string()));
        if let Err(error) = kept {
            tracing::warn!(%error, "could not keep the session catalog");
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemorySessionCatalog {
    kept: Mutex<std::collections::HashMap<String, Catalog>>,
}

impl SessionCatalog for InMemorySessionCatalog {
    fn read(&self, owner: &Principal) -> Catalog {
        self.kept.locked().get(&owner_key(owner)).cloned().unwrap_or_default()
    }

    fn update(&self, owner: &Principal, change: &mut dyn FnMut(&mut Catalog)) {
        change(self.kept.locked().entry(owner_key(owner)).or_default());
    }
}

pub(crate) struct Listing {
    catalog: std::sync::Arc<dyn SessionCatalog>,
    owner: Principal,
    cwd: PathBuf,
    session: Mutex<Option<domain::SessionId>>,
}

impl Listing {
    pub(crate) fn of(
        services: &crate::LocalServices,
        owner: &Principal,
        request: &domain::PromptRequest,
    ) -> Option<Self> {
        let chore = request.command.is_some_and(domain::SessionCommand::is_chore);
        (!chore).then(|| Self {
            catalog: std::sync::Arc::clone(&services.catalog),
            owner: owner.clone(),
            cwd: request.cwd.clone(),
            session: Mutex::new(None),
        })
    }

    pub(crate) fn observe(&self, event: &domain::EngineEvent) {
        use domain::EngineEvent as Event;
        use domain::conversation::SessionStatus as Status;
        if let Event::SessionStarted(session) = event {
            *self.session.locked() = Some(session.clone());
            self.catalog.update(&self.owner, &mut |catalog| catalog.started(session, &self.cwd, now()));
            return;
        }
        let status = match event {
            Event::Asked(_) => Status::NeedsYou,
            Event::AskWithdrawn(_) | Event::Answered { .. } => Status::Running,
            Event::Completed { .. } => Status::Done,
            Event::Failed(_) => Status::Failed,
            Event::Cancelled => Status::Cancelled,
            _ => return,
        };
        if let Some(session) = self.session.locked().clone() {
            self.catalog.update(&self.owner, &mut |catalog| catalog.status(&session, status, now()));
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use domain::{OrgId, SessionId, UserId};

    use super::*;

    #[test]
    fn each_owner_keeps_their_own_sessions_between_stores() {
        let root = tempfile::tempdir().expect("root");
        let (ana, rui) =
            (Principal::member(UserId::new(), OrgId::new()), Principal::member(UserId::new(), OrgId::new()));
        let store = FileSessionCatalog::new(root.path().to_path_buf());
        assert!(store.read(&ana).listed().is_empty(), "nothing kept yet");
        store.update(&ana, &mut |catalog| catalog.started(&SessionId("a".into()), Path::new("/w"), 1));

        let reopened = FileSessionCatalog::new(root.path().to_path_buf());
        assert_eq!(reopened.read(&ana).listed().len(), 1);
        assert!(reopened.read(&rui).listed().is_empty());
    }
}
