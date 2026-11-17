use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_engine::sessions::FileSessions;

use crate::catalog::{FileSessionCatalog, InMemorySessionCatalog, SessionCatalog};
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

    pub(crate) fn stores(&self) -> Stores {
        let sessions = Arc::new(FileSessions::new(self.sessions()));
        if self.keeps_nothing() {
            return Stores {
                settings: Arc::new(InMemorySettingsStore::default()),
                catalog: Arc::new(InMemorySessionCatalog::default()),
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
            settings: Arc::new(FileSettingsStore::new(self.users())),
            catalog: Arc::new(FileSessionCatalog::new(self.users())),
            providers: Arc::new(FileProviderStore::new(self.providers())),
            secrets,
            sessions,
        }
    }
}

pub(crate) struct Stores {
    pub settings: Arc<dyn SettingsStore>,
    pub catalog: Arc<dyn SessionCatalog>,
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
        assert_eq!(root.secrets(), Some(given.to_path_buf()));
        assert_eq!(root.sessions(), given.join("native-sessions"));
        assert_eq!(root.language_servers(), given.join("lsp"));
    }

    #[test]
    fn a_root_in_memory_keeps_no_vault_and_no_memory_and_its_folders_are_its_own() {
        let (root, other) = (DataRoot::in_memory(), DataRoot::in_memory());

        assert!(root.keeps_nothing());
        assert_eq!((root.secrets(), root.memory()), (None, None));
        assert!(root.folders().iter().all(|folder| root.holds(folder) && !other.holds(folder)));
        assert!(root.folders().iter().all(|folder| folder.starts_with(std::env::temp_dir()) && !folder.exists()));
        let system = DataRoot::system();
        assert!(root.folders().iter().all(|folder| !system.folders().contains(folder)));
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
        assert_eq!(root.memory(), app.map(|app| app.join("memory")));
        assert_eq!(root.secrets(), local.clone());
        assert_eq!(root.sessions(), local.clone().unwrap_or_else(fallback).join("native-sessions"));
        assert_eq!(root.language_servers(), local.unwrap_or_else(fallback).join("lsp"));
    }
}
