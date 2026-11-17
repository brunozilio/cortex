use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use domain::MemoryBook;
use domain::legacy::OWNER_DIR;

pub trait MemoryStore: Send + Sync {
    fn load(&self, project: &Path) -> MemoryBook;
    fn save(&self, project: &Path, book: &MemoryBook) -> std::io::Result<()>;

    fn update(&self, project: &Path, change: &mut dyn FnMut(&mut MemoryBook)) -> std::io::Result<MemoryBook> {
        let mut book = self.load(project);
        change(&mut book);
        self.save(project, &book)?;
        Ok(book)
    }
}

use crate::dirs::project_key;

pub fn memory_store_in(folder: Option<PathBuf>) -> Arc<dyn MemoryStore> {
    match folder {
        Some(folder) => Arc::new(FileMemoryStore::new(folder)),
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

    fn path(&self, project: &Path) -> PathBuf {
        self.root.join(OWNER_DIR).join(format!("{}.json", project_key(project)))
    }
}

impl MemoryStore for FileMemoryStore {
    fn load(&self, project: &Path) -> MemoryBook {
        let path = self.path(project);
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

    fn save(&self, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(book).map_err(std::io::Error::other)?;
        platform::private_file::replace(&self.path(project), &json)
    }

    fn update(&self, project: &Path, change: &mut dyn FnMut(&mut MemoryBook)) -> std::io::Result<MemoryBook> {
        let _held = platform::file_lock::Exclusive::on(&self.path(project))?;
        let mut book = self.load(project);
        change(&mut book);
        self.save(project, &book)?;
        Ok(book)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    books: Mutex<HashMap<PathBuf, MemoryBook>>,
}

impl MemoryStore for InMemoryStore {
    fn load(&self, project: &Path) -> MemoryBook {
        let books = self.books.locked();
        books.get(project).cloned().unwrap_or_default()
    }

    fn save(&self, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
        let mut books = self.books.locked();
        books.insert(project.to_path_buf(), book.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_s_memory_stays_in_the_local_folder() {
        let data = tempfile::tempdir().expect("data");
        let store = FileMemoryStore::new(data.path().join("memory"));

        store.save(Path::new("/work/project"), &MemoryBook::default()).expect("save");

        assert!(data.path().join("memory/local/-work-project.json").is_file());
    }
}
