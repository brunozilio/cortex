use std::path::PathBuf;
use std::sync::Mutex;

use platform::store::Document;
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
    book: Document,
}

impl FileProviderStore {
    pub fn new(root: PathBuf) -> Self {
        Self { book: Document::at(root.join(OWNER_DIR).join("providers.json")) }
    }
}

impl ProviderStore for FileProviderStore {
    fn load(&self) -> ProviderBook {
        self.book.read_or_default()
    }

    fn save(&self, book: &ProviderBook) -> std::io::Result<()> {
        Ok(self.book.write(book)?)
    }

    fn update(&self, change: &mut dyn FnMut(&mut ProviderBook)) -> std::io::Result<()> {
        Ok(self.book.update(|book: &mut ProviderBook| change(book))?)
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

    #[test]
    fn a_broken_file_is_kept_aside_before_anything_is_written_in_its_place() {
        let dir = tempfile::tempdir().expect("dir");
        let store = FileProviderStore::new(dir.path().to_path_buf());
        let folder = dir.path().join("local");
        std::fs::create_dir_all(&folder).expect("folder");
        std::fs::write(folder.join("providers.json"), b"{not json").expect("write");

        let account = ProviderAccount { label: "…ab12".into(), since_unix: 5, id: String::new() };
        store.update(&mut |book| book.connect(ModelProvider::new("keyed"), account.clone())).expect("update");

        assert_ne!(store.load(), ProviderBook::default());
        let aside: Vec<_> = std::fs::read_dir(&folder)
            .expect("list")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("providers.json.corrupt-"))
            .collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read(aside[0].path()).expect("aside"), b"{not json");
    }
}
