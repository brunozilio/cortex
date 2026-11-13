use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use platform::sync::Locked;

use domain::Principal;
use domain::provider::ProviderBook;

use crate::dirs::{app_data_dir, owner_key};

pub trait ProviderStore: Send + Sync {
    fn load(&self, owner: &Principal) -> ProviderBook;
    fn save(&self, owner: &Principal, book: &ProviderBook) -> std::io::Result<()>;

    fn update(&self, owner: &Principal, change: &mut dyn FnMut(&mut ProviderBook)) -> std::io::Result<()> {
        let mut book = self.load(owner);
        change(&mut book);
        self.save(owner, &book)
    }
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
        let json = serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?;
        platform::private_file::replace(&self.book(owner), &json)
    }

    fn update(&self, owner: &Principal, change: &mut dyn FnMut(&mut ProviderBook)) -> std::io::Result<()> {
        let _held = platform::file_lock::Exclusive::on(&self.book(owner))?;
        let mut book = self.load(owner);
        change(&mut book);
        self.save(owner, &book)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryProviderStore {
    books: Mutex<HashMap<String, ProviderBook>>,
}

impl ProviderStore for InMemoryProviderStore {
    fn load(&self, owner: &Principal) -> ProviderBook {
        let books = self.books.locked();
        books.get(&owner_key(owner)).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, book: &ProviderBook) -> std::io::Result<()> {
        let mut books = self.books.locked();
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
        book.connect(ModelProvider::new("keyed"), account);
        store.save(&owner, &book).expect("save");
        assert_eq!(store.load(&owner), book);
        let other = Principal::member(domain::UserId::new(), domain::OrgId::new());
        assert_eq!(store.load(&other), ProviderBook::default());
        std::fs::write(dir.path().join("local").join("providers.json"), b"{not json").expect("write");
        assert_eq!(store.load(&owner), ProviderBook::default());
    }
}
