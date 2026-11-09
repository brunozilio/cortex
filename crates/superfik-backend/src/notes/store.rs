//! Where note books are kept: one JSON file per owner and project, or in
//! memory for tests.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use superfik_core::Principal;
use superfik_core::note::NoteBook;

use crate::memory::{app_data_dir, owner_key, project_key};

pub trait NoteStore: Send + Sync {
    fn load(&self, owner: &Principal, project: &Path) -> NoteBook;
    fn save(&self, owner: &Principal, project: &Path, book: &NoteBook) -> std::io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct FileNoteStore {
    root: PathBuf,
}

impl FileNoteStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("notes"))
    }

    fn path(&self, owner: &Principal, project: &Path) -> PathBuf {
        self.root.join(owner_key(owner)).join(format!("{}.json", project_key(project)))
    }
}

impl NoteStore for FileNoteStore {
    fn load(&self, owner: &Principal, project: &Path) -> NoteBook {
        std::fs::read(self.path(owner, project))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, owner: &Principal, project: &Path, book: &NoteBook) -> std::io::Result<()> {
        let path = self.path(owner, project);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json)?;
        std::fs::rename(partial, path)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryNoteStore {
    books: Mutex<HashMap<(String, PathBuf), NoteBook>>,
}

impl NoteStore for InMemoryNoteStore {
    fn load(&self, owner: &Principal, project: &Path) -> NoteBook {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.get(&(owner_key(owner), project.to_path_buf())).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, project: &Path, book: &NoteBook) -> std::io::Result<()> {
        let mut books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.insert((owner_key(owner), project.to_path_buf()), book.clone());
        Ok(())
    }
}
