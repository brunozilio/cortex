use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::sessions::FileSessions;

use crate::catalog::{InMemorySessionCatalog, SessionCatalog, SqliteSessionCatalog};
use crate::db::Db;
use crate::degraded::Degraded;
use crate::features::memory::Memories;
use crate::providers::{FileProviderStore, InMemoryProviderStore, ProviderStore};
use crate::secrets::{InMemorySecretStore, SecretStore};
use crate::settings::{FileSettingsStore, InMemorySettingsStore, SettingsStore};

const SESSIONS: &str = "native-sessions";
const LANGUAGE_SERVERS: &str = "lsp";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kept {
    System { app: Option<PathBuf>, local: Option<PathBuf> },
    Under(PathBuf),
    InMemory { scratch: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot(Kept);

impl DataRoot {
    pub fn system() -> Self {
        Self(Kept::System { app: crate::dirs::app_data_dir(), local: crate::dirs::local_data_dir() })
    }

    pub fn under(folder: impl Into<PathBuf>) -> Self {
        Self(Kept::Under(folder.into()))
    }

    pub fn in_memory() -> Self {
        let scratch = format!("{}-in-memory-{}", brand::COMMAND, uuid::Uuid::new_v4().simple());
        Self(Kept::InMemory { scratch: std::env::temp_dir().join(scratch) })
    }

    pub fn keeps_nothing(&self) -> bool {
        matches!(self.0, Kept::InMemory { .. })
    }

    fn app(&self) -> PathBuf {
        match &self.0 {
            Kept::System { app, .. } => app.clone().unwrap_or_else(std::env::temp_dir),
            Kept::Under(folder) => folder.clone(),
            Kept::InMemory { scratch } => scratch.clone(),
        }
    }

    fn local(&self) -> PathBuf {
        match &self.0 {
            Kept::System { local, .. } => local.clone().unwrap_or_else(std::env::temp_dir),
            Kept::Under(folder) => folder.clone(),
            Kept::InMemory { scratch } => scratch.clone(),
        }
    }

    pub fn users(&self) -> PathBuf {
        self.app().join("users")
    }

    pub fn providers(&self) -> PathBuf {
        self.app().join("providers")
    }

    pub fn code_map(&self) -> PathBuf {
        self.app().join("code-map")
    }

    pub fn memory(&self) -> Option<PathBuf> {
        match &self.0 {
            Kept::System { app, .. } => app.as_ref().map(|app| app.join("memory")),
            Kept::Under(folder) => Some(folder.join("memory")),
            Kept::InMemory { .. } => None,
        }
    }

    pub fn secrets(&self) -> Option<PathBuf> {
        match &self.0 {
            Kept::System { local, .. } => local.clone(),
            Kept::Under(folder) => Some(folder.clone()),
            Kept::InMemory { .. } => None,
        }
    }

    pub fn sessions(&self) -> PathBuf {
        self.local().join(SESSIONS)
    }

    pub fn language_servers(&self) -> PathBuf {
        self.local().join(LANGUAGE_SERVERS)
    }

    pub fn folders(&self) -> Vec<PathBuf> {
        [self.users(), self.providers(), self.code_map(), self.sessions(), self.language_servers()]
            .into_iter()
            .chain(self.memory())
            .chain(self.secrets())
            .collect()
    }

    pub fn holds(&self, path: &Path) -> bool {
        match &self.0 {
            Kept::System { .. } => self.folders().iter().any(|folder| path.starts_with(folder)),
            Kept::Under(folder) => path.starts_with(folder),
            Kept::InMemory { scratch } => path.starts_with(scratch),
        }
    }

    pub fn database(&self) -> Option<PathBuf> {
        match &self.0 {
            Kept::System { app, .. } => app.as_ref().map(|app| app.join(crate::db::FILE)),
            Kept::Under(folder) => Some(folder.join(crate::db::FILE)),
            Kept::InMemory { .. } => None,
        }
    }

    fn db(&self) -> (Option<Db>, Degraded) {
        let Some(file) = self.database() else {
            if self.keeps_nothing() {
                return (in_memory(), Degraded::default());
            }
            tracing::error!("no app data folder; sessions and memory last only until the process ends");
            return (in_memory(), Degraded::because("this user has no data folder"));
        };
        match Db::open(&file) {
            Ok(db) => {
                crate::catalog::legacy::import(&db, &self.users());
                if let Some(memory) = self.memory() {
                    crate::features::memory::store::legacy::import(&db, &memory);
                }
                (Some(db), Degraded::default())
            }
            Err(error) => {
                tracing::error!(%error, "the database could not be opened; sessions and memory last only until the process ends");
                (in_memory(), Degraded::because(error.to_string()))
            }
        }
    }

    pub(crate) fn stores(&self) -> Stores {
        let sessions = Arc::new(FileSessions::new(self.sessions()));
        let (db, database) = self.db();
        let (catalog, memories): (Arc<dyn SessionCatalog>, Memories) = match db {
            Some(db) => (Arc::new(SqliteSessionCatalog::new(db.clone())), Memories::kept_in(db)),
            None => (Arc::new(InMemorySessionCatalog::default()), Memories::only_in_the_process()),
        };
        if self.keeps_nothing() {
            return Stores {
                trusts: Arc::new(crate::trust::InMemoryTrustStore::default()),
                settings: Arc::new(InMemorySettingsStore::default()),
                catalog,
                memories,
                database,
                providers: Arc::new(InMemoryProviderStore::default()),
                secrets: Arc::new(InMemorySecretStore::default()),
                sessions,
            };
        }
        let secrets = match &self.0 {
            Kept::System { .. } => crate::secrets::system_secrets(),
            _ => crate::secrets::secrets_in(self.secrets().as_deref()),
        };
        Stores {
            trusts: Arc::new(crate::trust::FileTrustStore::new(self.users())),
            settings: Arc::new(FileSettingsStore::new(self.users())),
            catalog,
            memories,
            database,
            providers: Arc::new(FileProviderStore::new(self.providers())),
            secrets,
            sessions,
        }
    }
}

fn in_memory() -> Option<Db> {
    Db::in_memory().map_err(|error| tracing::error!(%error, "no database could be opened, not even in memory")).ok()
}

pub(crate) struct Stores {
    pub trusts: Arc<dyn crate::trust::TrustStore>,
    pub settings: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn SessionCatalog>,
    pub memories: Memories,
    pub database: Degraded,
    pub providers: Arc<dyn ProviderStore>,
    pub secrets: Arc<dyn SecretStore>,
    pub sessions: Arc<FileSessions>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_given_folder_holds_every_folder_of_the_root() {
        let given = Path::new("/given/data");
        let root = DataRoot::under(given);

        assert_eq!(root.folders().len(), 7);
        assert!(root.folders().iter().all(|folder| folder.starts_with(given)), "{:?}", root.folders());
        assert_eq!(root.users(), given.join("users"));
        assert_eq!(root.providers(), given.join("providers"));
        assert_eq!(root.memory(), Some(given.join("memory")));
        assert_eq!(root.database(), Some(given.join("app.db")));
        assert_eq!(root.secrets(), Some(given.to_path_buf()));
        assert_eq!(root.sessions(), given.join("native-sessions"));
        assert_eq!(root.language_servers(), given.join("lsp"));
    }

    #[test]
    fn a_root_in_memory_keeps_no_vault_and_no_memory_and_its_folders_are_its_own() {
        let (root, other) = (DataRoot::in_memory(), DataRoot::in_memory());

        assert!(root.keeps_nothing());
        assert_eq!((root.secrets(), root.memory(), root.database()), (None, None, None));
        assert!(root.folders().iter().all(|folder| root.holds(folder) && !other.holds(folder)));
        assert!(root.folders().iter().all(|folder| folder.starts_with(std::env::temp_dir()) && !folder.exists()));
        let system = DataRoot::system();
        assert!(root.folders().iter().all(|folder| !system.folders().contains(folder)));
    }

    #[test]
    fn a_database_that_cannot_be_opened_is_said_with_its_reason_and_one_that_opens_or_is_not_wanted_is_not() {
        let data = tempfile::tempdir().expect("data");
        assert_eq!(DataRoot::under(data.path()).stores().database.reason(), None);
        assert_eq!(DataRoot::in_memory().stores().database.reason(), None, "nothing was meant to be kept");

        let newer = rusqlite::Connection::open(data.path().join(crate::db::FILE)).expect("open");
        newer.execute("INSERT INTO schema_version (version) VALUES (9999)", []).expect("a later schema");
        drop(newer);
        let stores = DataRoot::under(data.path()).stores();
        let reason = stores.database.reason().expect("degraded").to_owned();
        assert!(reason.contains("app.db") && reason.contains("newer version"), "{reason}");
        stores.catalog.update(&mut |catalog| {
            catalog.started(&domain::SessionId("s".into()), Path::new("/work"), 1);
        });
        assert_eq!(stores.catalog.read().listed().len(), 1, "the session list still works, in the process only");

        let homeless = DataRoot(Kept::System { app: None, local: None });
        assert_eq!(homeless.stores().database.reason(), Some("this user has no data folder"));
    }

    #[test]
    fn the_system_root_is_where_data_has_always_been() {
        let root = DataRoot::system();
        let app = crate::dirs::app_data_dir();
        let local = crate::dirs::local_data_dir();
        let fallback = std::env::temp_dir;

        assert_eq!(root.users(), app.clone().unwrap_or_else(fallback).join("users"));
        assert_eq!(root.providers(), app.clone().unwrap_or_else(fallback).join("providers"));
        assert_eq!(root.code_map(), app.clone().unwrap_or_else(fallback).join("code-map"));
        assert_eq!(root.memory(), app.clone().map(|app| app.join("memory")));
        assert_eq!(root.database(), app.map(|app| app.join("app.db")));
        assert_eq!(root.secrets(), local.clone());
        assert_eq!(root.sessions(), local.clone().unwrap_or_else(fallback).join("native-sessions"));
        assert_eq!(root.language_servers(), local.unwrap_or_else(fallback).join("lsp"));
    }
}
