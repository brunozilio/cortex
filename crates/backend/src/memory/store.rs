use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::{MemoryBook, Principal};

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

pub fn system_memory_store() -> Arc<dyn MemoryStore> {
    memory_store_in(app_data_dir())
}

pub fn memory_store_in(data: Option<PathBuf>) -> Arc<dyn MemoryStore> {
    match data {
        Some(data) => Arc::new(FileMemoryStore::new(data.join("memory"))),
        None => {
            tracing::error!("no app data folder; memory lasts only until the process ends");
            Arc::new(InMemoryStore::default())
        }
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

    fn path(&self, owner: &Principal, project: &Path) -> PathBuf {
        self.root.join(owner_key(owner)).join(format!("{}.json", project_key(project)))
    }
}

impl MemoryStore for FileMemoryStore {
    fn load(&self, owner: &Principal, project: &Path) -> MemoryBook {
        let path = self.path(owner, project);
        let Ok(bytes) = std::fs::read(&path) else { return MemoryBook::default() };
        match serde_json::from_slice(&bytes) {
            Ok(book) => book,
            Err(error) => {
                let aside = path.with_extension(format!("json.corrupt-{}", super::now()));
                match std::fs::rename(&path, &aside) {
                    Ok(()) => tracing::warn!(%error, aside = %aside.display(), "unreadable memory kept aside"),
                    Err(rename) => tracing::error!(%error, %rename, "unreadable memory could not be kept aside"),
                }
                MemoryBook::default()
            }
        }
    }

    fn save(&self, owner: &Principal, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
        use std::io::Write;
        let path = self.path(owner, project);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?;
        let partial = path.with_extension("json.partial");
        let mut file = std::fs::File::create(&partial)?;
        file.write_all(&json)?;
        file.sync_all()?;
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
