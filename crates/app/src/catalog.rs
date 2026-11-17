use std::path::PathBuf;
use std::sync::Mutex;

use domain::catalog::Catalog;
use domain::legacy::OWNER_DIR;
use platform::store::Document;
use platform::sync::Locked;

pub trait SessionCatalog: Send + Sync {
    fn read(&self) -> Catalog;
    fn update(&self, change: &mut dyn FnMut(&mut Catalog));
}

#[derive(Debug)]
pub struct FileSessionCatalog {
    kept: Document,
}

impl FileSessionCatalog {
    pub fn new(root: PathBuf) -> Self {
        Self { kept: Document::at(root.join(OWNER_DIR).join("sessions.json")).compact() }
    }
}

impl SessionCatalog for FileSessionCatalog {
    fn read(&self) -> Catalog {
        self.kept.read_or_default()
    }

    fn update(&self, change: &mut dyn FnMut(&mut Catalog)) {
        if let Err(error) = self.kept.update(|catalog: &mut Catalog| change(catalog)) {
            tracing::warn!(%error, "could not keep the session catalog; it was left as it was");
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

const RECENT: usize = 30;

pub(crate) fn recent_sessions(catalog: &Catalog, cwd: &std::path::Path) -> Vec<domain::SessionId> {
    catalog.listed().iter().filter(|row| row.cwd == cwd).take(RECENT).map(|row| row.session.clone()).collect()
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
    fn a_folder_s_recent_sessions_are_its_own_newest_first() {
        let mut catalog = Catalog::default();
        catalog.started(&SessionId("old".into()), Path::new("/w"), 1);
        catalog.started(&SessionId("elsewhere".into()), Path::new("/other"), 2);
        catalog.started(&SessionId("new".into()), Path::new("/w"), 3);
        assert_eq!(recent_sessions(&catalog, Path::new("/w")), [SessionId("new".into()), SessionId("old".into())]);
        assert!(recent_sessions(&catalog, Path::new("/nowhere")).is_empty());
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
