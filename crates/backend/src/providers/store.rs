use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use domain::Principal;
use domain::provider::ProviderBook;

use crate::memory::{app_data_dir, owner_key};

pub trait ProviderStore: Send + Sync {
    fn load(&self, owner: &Principal) -> ProviderBook;
    fn save(&self, owner: &Principal, book: &ProviderBook) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct FileProviderStore {
    root: PathBuf,
}

impl FileProviderStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("providers"))
    }

    fn book(&self, owner: &Principal) -> PathBuf {
        self.root.join(owner_key(owner)).join("providers.json")
    }
}

impl ProviderStore for FileProviderStore {
    fn load(&self, owner: &Principal) -> ProviderBook {
        let path = self.book(owner);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, path = %path.display(), "unreadable providers file");
                ProviderBook::default()
            }),
            Err(_) => ProviderBook::default(),
        }
    }

    fn save(&self, owner: &Principal, book: &ProviderBook) -> std::io::Result<()> {
        let path = self.book(owner);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?)?;
        std::fs::rename(partial, path)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryProviderStore {
    books: Mutex<HashMap<String, ProviderBook>>,
}

impl ProviderStore for InMemoryProviderStore {
    fn load(&self, owner: &Principal) -> ProviderBook {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.get(&owner_key(owner)).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, book: &ProviderBook) -> std::io::Result<()> {
        let mut books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.insert(owner_key(owner), book.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use domain::provider::{ModelProvider, ProviderAccount};

    use super::*;

    #[test]
    fn file_store_round_trips_per_owner_and_forgives_a_broken_file() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FileProviderStore::new(dir.path().to_path_buf());
        let owner = Principal::LocalOwner;
        assert_eq!(store.load(&owner), ProviderBook::default());
        let mut book = ProviderBook::default();
        let account = ProviderAccount { label: "…ab12".into(), since_unix: 5 };
        book.connect(ModelProvider::OpenAi, account);
        store.save(&owner, &book).expect("save");
        assert_eq!(store.load(&owner), book);
        let other = Principal::member(domain::UserId::new(), domain::OrgId::new());
        assert_eq!(store.load(&other), ProviderBook::default());
        std::fs::write(dir.path().join("local").join("providers.json"), b"{not json").expect("write");
        assert_eq!(store.load(&owner), ProviderBook::default());
    }
}
