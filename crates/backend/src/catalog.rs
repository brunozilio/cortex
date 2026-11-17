use std::path::PathBuf;
use std::sync::Mutex;

use domain::catalog::Catalog;
use domain::legacy::OWNER_DIR;
use platform::sync::Locked;

pub trait SessionCatalog: Send + Sync {
    fn read(&self) -> Catalog;
    fn update(&self, change: &mut dyn FnMut(&mut Catalog));
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

    fn path(&self) -> PathBuf {
        self.root.join(OWNER_DIR).join("sessions.json")
    }

    fn load(&self) -> Catalog {
        std::fs::read(self.path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    fn load_to_change(&self) -> Catalog {
        let path = self.path();
        let Ok(bytes) = std::fs::read(&path) else { return Catalog::default() };
        serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            let aside = path.with_extension(format!("json.corrupt-{}", now()));
            match std::fs::rename(&path, &aside) {
                Ok(()) => {
                    tracing::warn!(%error, aside = %aside.display(), "an unreadable session catalog was kept aside")
                }
                Err(rename) => {
                    tracing::error!(%error, %rename, "an unreadable session catalog could not be kept aside")
                }
            }
            Catalog::default()
        })
    }
}

impl SessionCatalog for FileSessionCatalog {
    fn read(&self) -> Catalog {
        let _writing = self.writing.locked();
        self.load()
    }

    fn update(&self, change: &mut dyn FnMut(&mut Catalog)) {
        let _writing = self.writing.locked();
        let _held = match platform::file_lock::Exclusive::on(&self.path()) {
            Ok(held) => held,
            Err(error) => {
                tracing::warn!(%error, "could not lock the session catalog; it was left as it was");
                return;
            }
        };
        let before = self.load_to_change();
        let mut catalog = before.clone();
        change(&mut catalog);
        if catalog == before {
            return;
        }
        let kept = serde_json::to_vec(&catalog)
            .map_err(|error| error.to_string())
            .and_then(|json| platform::private_file::replace(&self.path(), &json).map_err(|e| e.to_string()));
        if let Err(error) = kept {
            tracing::warn!(%error, "could not keep the session catalog");
        }
    }
}

#[derive(Debug, Default)]
pub struct InMemorySessionCatalog {
    kept: Mutex<Catalog>,
}

impl SessionCatalog for InMemorySessionCatalog {
    fn read(&self) -> Catalog {
        self.kept.locked().clone()
    }

    fn update(&self, change: &mut dyn FnMut(&mut Catalog)) {
        change(&mut self.kept.locked());
    }
}

pub(crate) struct Listing {
    catalog: std::sync::Arc<dyn SessionCatalog>,
    cwd: PathBuf,
    session: Mutex<Option<domain::SessionId>>,
}

impl Listing {
    pub(crate) fn of(services: &crate::LocalServices, request: &domain::PromptRequest) -> Option<Self> {
        let chore = request.command.is_some_and(domain::SessionCommand::is_chore);
        (!chore).then(|| Self {
            catalog: std::sync::Arc::clone(&services.catalog),
            cwd: request.cwd.clone(),
            session: Mutex::new(None),
        })
    }

    pub(crate) fn observe(&self, event: &domain::EngineEvent) {
        use domain::EngineEvent as Event;
        use domain::conversation::SessionStatus as Status;
        if let Event::SessionStarted(session) = event {
            *self.session.locked() = Some(session.clone());
            self.catalog.update(&mut |catalog| catalog.started(session, &self.cwd, now()));
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
            self.catalog.update(&mut |catalog| catalog.status(&session, status, now()));
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use domain::SessionId;

    use super::*;

    #[test]
    fn sessions_are_kept_between_stores() {
        let root = tempfile::tempdir().expect("root");
        let store = FileSessionCatalog::new(root.path().to_path_buf());
        assert!(store.read().listed().is_empty(), "nothing kept yet");
        store.update(&mut |catalog| catalog.started(&SessionId("a".into()), Path::new("/w"), 1));

        let reopened = FileSessionCatalog::new(root.path().to_path_buf());
        assert_eq!(reopened.read().listed().len(), 1);
    }

    #[test]
    fn a_catalog_that_does_not_parse_is_kept_aside_instead_of_overwritten() {
        let root = tempfile::tempdir().expect("root");
        let store = FileSessionCatalog::new(root.path().to_path_buf());
        let folder = root.path().join(OWNER_DIR);
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("sessions.json"), "{ half written").expect("write");

        store.update(&mut |catalog| catalog.started(&SessionId("a".into()), Path::new("/w"), 1));

        assert_eq!(store.read().listed().len(), 1);
        let aside: Vec<_> = std::fs::read_dir(&folder)
            .expect("list")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("sessions.json.corrupt-"))
            .collect();
        assert_eq!(aside.len(), 1, "what could not be read is still there to recover");
        assert_eq!(std::fs::read_to_string(aside[0].path()).expect("aside"), "{ half written");
    }

    #[test]
    fn two_processes_on_one_catalog_keep_each_other_s_sessions() {
        let root = tempfile::tempdir().expect("root");
        let start = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            for writer in 0..8 {
                let (root, start) = (root.path().to_path_buf(), &start);
                scope.spawn(move || {
                    let store = FileSessionCatalog::new(root);
                    start.wait();
                    for session in 0..5 {
                        let id = SessionId(format!("{writer}-{session}"));
                        store.update(&mut |catalog| catalog.started(&id, Path::new("/w"), 1));
                    }
                });
            }
        });
        let kept = FileSessionCatalog::new(root.path().to_path_buf()).read().listed().len();
        assert_eq!(kept, 40);
    }
}
