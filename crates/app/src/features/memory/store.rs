pub(crate) mod legacy;
mod sqlite;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use platform::sync::Locked;

use domain::MemoryBook;

pub use sqlite::SqliteMemoryStore;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Revised {
    pub revision: u64,
    pub book: MemoryBook,
}

pub trait MemoryStore: Send + Sync {
    fn revision(&self, project: &Path) -> u64;
    fn read(&self, project: &Path) -> Revised;
    fn update(&self, project: &Path, change: &mut dyn FnMut(&mut MemoryBook)) -> std::io::Result<Revised>;
    fn undo_point(&self, project: &Path) -> Option<MemoryBook>;
    fn set_undo_point(&self, project: &Path, book: Option<&MemoryBook>) -> std::io::Result<()>;

    fn load(&self, project: &Path) -> MemoryBook {
        self.read(project).book
    }

    fn save(&self, project: &Path, book: &MemoryBook) -> std::io::Result<()> {
        self.update(project, &mut |kept| *kept = book.clone()).map(drop)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    books: Mutex<HashMap<PathBuf, Revised>>,
    undo_points: Mutex<HashMap<PathBuf, MemoryBook>>,
}

impl MemoryStore for InMemoryStore {
    fn revision(&self, project: &Path) -> u64 {
        self.books.locked().get(project).map_or(0, |kept| kept.revision)
    }

    fn read(&self, project: &Path) -> Revised {
        self.books.locked().get(project).cloned().unwrap_or_default()
    }

    fn update(&self, project: &Path, change: &mut dyn FnMut(&mut MemoryBook)) -> std::io::Result<Revised> {
        let mut books = self.books.locked();
        let kept = books.entry(project.to_path_buf()).or_default();
        let before = kept.book.clone();
        change(&mut kept.book);
        if kept.book != before {
            kept.revision += 1;
        }
        Ok(kept.clone())
    }

    fn undo_point(&self, project: &Path) -> Option<MemoryBook> {
        self.undo_points.locked().get(project).cloned()
    }

    fn set_undo_point(&self, project: &Path, book: Option<&MemoryBook>) -> std::io::Result<()> {
        let mut points = self.undo_points.locked();
        match book {
            Some(book) => points.insert(project.to_path_buf(), book.clone()),
            None => points.remove(project),
        };
        Ok(())
    }
}
