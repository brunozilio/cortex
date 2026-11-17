use std::path::PathBuf;
use std::sync::Mutex;

use platform::sync::Locked;

use domain::legacy::OWNER_DIR;
use domain::provider::ProviderBook;

pub trait ProviderStore: Send + Sync {
    fn load(&self) -> ProviderBook;
    fn save(&self, book: &ProviderBook) -> std::io::Result<()>;

    fn update(&self, change: &mut dyn FnMut(&mut ProviderBook)) -> std::io::Result<()> {
        let mut book = self.load();
        change(&mut book);
        self.save(&book)
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

    fn book(&self) -> PathBuf {
        self.root.join(OWNER_DIR).join("providers.json")
    }
}

impl ProviderStore for FileProviderStore {
    fn load(&self) -> ProviderBook {
        let path = self.book();
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, path = %path.display(), "unreadable providers file");
                ProviderBook::default()
            }),
            Err(_) => ProviderBook::default(),
        }
    }

    fn save(&self, book: &ProviderBook) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?;
        platform::private_file::replace(&self.book(), &json)
    }

    fn update(&self, change: &mut dyn FnMut(&mut ProviderBook)) -> std::io::Result<()> {
        let _held = platform::file_lock::Exclusive::on(&self.book())?;
        let mut book = self.load();
        change(&mut book);
        self.save(&book)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryProviderStore {
    book: Mutex<ProviderBook>,
}

impl ProviderStore for InMemoryProviderStore {
    fn load(&self) -> ProviderBook {
        self.book.locked().clone()
    }

    fn save(&self, book: &ProviderBook) -> std::io::Result<()> {
        *self.book.locked() = book.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use domain::provider::{ModelProvider, ProviderAccount};

    use super::*;

    #[test]
    fn file_store_round_trips_and_forgives_a_broken_file() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FileProviderStore::new(dir.path().to_path_buf());
        assert_eq!(store.load(), ProviderBook::default());
        let mut book = ProviderBook::default();
        let account = ProviderAccount { label: "…ab12".into(), since_unix: 5, id: String::new() };
        book.connect(ModelProvider::new("keyed"), account);
        store.save(&book).expect("save");
        assert_eq!(store.load(), book);
        std::fs::write(dir.path().join("local").join("providers.json"), b"{not json").expect("write");
        assert_eq!(store.load(), ProviderBook::default());
    }
}
