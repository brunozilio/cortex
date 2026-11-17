use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use platform::store::Document;
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

    fn book(&self, project: &Path) -> Document {
        Document::at(self.root.join(OWNER_DIR).join(format!("{}.json", project_key(project))))
    }
}

impl MemoryStore for FileMemoryStore {
    fn load(&self, project: &Path) -> MemoryBook {
        self.book(project).read_or_default()
    }

    fn save(&self, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
        Ok(self.book(project).write(book)?)
    }

    fn update(&self, project: &Path, change: &mut dyn FnMut(&mut MemoryBook)) -> std::io::Result<MemoryBook> {
        let changed = self.book(project).update(|book: &mut MemoryBook| {
            change(book);
            book.clone()
        });
        Ok(changed?)
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

    #[test]
    fn stores_updating_one_project_at_once_lose_none_of_each_other_s_changes() {
        let data = tempfile::tempdir().expect("data");
        let project = Path::new("/work/project");
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let store = FileMemoryStore::new(data.path().to_path_buf());
                    for _ in 0..10 {
                        let changed = store.update(project, &mut |book| {
                            book.scorecard.next_held_out();
                        });
                        changed.expect("update");
                    }
                });
            }
        });
        let kept = FileMemoryStore::new(data.path().to_path_buf()).load(project);
        assert_eq!(kept.scorecard.briefed, 40);
    }
}
