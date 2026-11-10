//! Where each owner's [`PluginBook`] and installed plugin folders live.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use superfik_core::Principal;
use superfik_plugins::PluginBook;

use crate::memory::{app_data_dir, owner_key};

pub trait PluginStore: Send + Sync {
    fn load(&self, owner: &Principal) -> PluginBook;
    fn save(&self, owner: &Principal, book: &PluginBook) -> std::io::Result<()>;
    fn packages(&self, owner: &Principal) -> PathBuf;
}

#[derive(Debug, Clone)]
pub struct FilePluginStore {
    root: PathBuf,
}

impl FilePluginStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("plugins"))
    }

    fn book(&self, owner: &Principal) -> PathBuf {
        self.root.join(owner_key(owner)).join("plugins.json")
    }
}

impl PluginStore for FilePluginStore {
    fn load(&self, owner: &Principal) -> PluginBook {
        let path = self.book(owner);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, path = %path.display(), "unreadable plugins file");
                PluginBook::default()
            }),
            Err(_) => PluginBook::default(),
        }
    }

    fn save(&self, owner: &Principal, book: &PluginBook) -> std::io::Result<()> {
        let path = self.book(owner);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?)?;
        std::fs::rename(partial, path)
    }

    fn packages(&self, owner: &Principal) -> PathBuf {
        self.root.join(owner_key(owner)).join("packages")
    }
}

#[derive(Debug)]
pub struct InMemoryPluginStore {
    books: Mutex<HashMap<String, PluginBook>>,
    packages: PathBuf,
}

impl InMemoryPluginStore {
    pub fn new(packages: PathBuf) -> Self {
        Self { books: Mutex::default(), packages }
    }
}

impl PluginStore for InMemoryPluginStore {
    fn load(&self, owner: &Principal) -> PluginBook {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.get(&owner_key(owner)).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, book: &PluginBook) -> std::io::Result<()> {
        let mut books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.insert(owner_key(owner), book.clone());
        Ok(())
    }

    fn packages(&self, owner: &Principal) -> PathBuf {
        self.packages.join(owner_key(owner))
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{PluginId, PluginOrigin};

    use super::*;

    #[test]
    fn file_store_round_trips_per_owner() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FilePluginStore::new(dir.path().to_path_buf());
        let owner = Principal::LocalOwner;
        assert_eq!(store.load(&owner), PluginBook::default());
        let mut book = PluginBook::default();
        book.install(PluginId("linear".into()), PluginOrigin::Local("/p".into())).expect("install");
        store.save(&owner, &book).expect("save");
        assert_eq!(store.load(&owner), book);
        assert_eq!(
            store.load(&Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new())),
            PluginBook::default()
        );
        assert!(store.packages(&owner).ends_with("local/packages"));
    }
}
