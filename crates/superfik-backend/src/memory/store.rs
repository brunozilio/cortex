//! Where memory books are kept: one JSON file per owner and project, or
//! in memory for tests.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use superfik_core::{MemoryBook, Principal};

pub trait MemoryStore: Send + Sync {
    fn load(&self, owner: &Principal, project: &Path) -> MemoryBook;
    fn save(&self, owner: &Principal, project: &Path, book: &MemoryBook) -> std::io::Result<()>;
}

pub use crate::dirs::app_data_dir;

pub fn project_key(project: &Path) -> String {
    project.to_string_lossy().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect()
}

pub fn owner_key(owner: &Principal) -> String {
    match owner {
        Principal::LocalOwner => "local".to_owned(),
        Principal::Member(member) => format!("org-{}-user-{}", member.org, member.user),
    }
}

#[derive(Debug, Clone)]
pub struct FileMemoryStore {
    root: PathBuf,
}

impl FileMemoryStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("memory"))
    }

    fn path(&self, owner: &Principal, project: &Path) -> PathBuf {
        self.root.join(owner_key(owner)).join(format!("{}.json", project_key(project)))
    }
}

impl MemoryStore for FileMemoryStore {
    fn load(&self, owner: &Principal, project: &Path) -> MemoryBook {
        std::fs::read(self.path(owner, project))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, owner: &Principal, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
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
pub struct InMemoryStore {
    books: Mutex<HashMap<(String, PathBuf), MemoryBook>>,
}

impl MemoryStore for InMemoryStore {
    fn load(&self, owner: &Principal, project: &Path) -> MemoryBook {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.get(&(owner_key(owner), project.to_path_buf())).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
        let mut books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.insert((owner_key(owner), project.to_path_buf()), book.clone());
        Ok(())
    }
}
