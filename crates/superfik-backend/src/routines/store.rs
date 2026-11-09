//! Where routine books are kept: one JSON file per owner and project (which
//! also names them, so the scheduler can find every book at start), or in
//! memory for tests.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use superfik_core::Principal;
use superfik_core::routine::RoutineBook;

use crate::memory::{app_data_dir, owner_key, project_key};

pub trait RoutineStore: Send + Sync {
    fn load(&self, owner: &Principal, project: &Path) -> RoutineBook;
    fn save(&self, owner: &Principal, project: &Path, book: &RoutineBook) -> std::io::Result<()>;
    fn all(&self) -> Vec<(Principal, PathBuf)>;
}

#[derive(Serialize, Deserialize)]
struct Stored {
    owner: Principal,
    project: PathBuf,
    book: RoutineBook,
}

#[derive(Debug, Clone)]
pub struct FileRoutineStore {
    root: PathBuf,
}

impl FileRoutineStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn system() -> Self {
        Self::new(app_data_dir().unwrap_or_else(std::env::temp_dir).join("routines"))
    }

    fn path(&self, owner: &Principal, project: &Path) -> PathBuf {
        self.root.join(owner_key(owner)).join(format!("{}.json", project_key(project)))
    }

    fn read(path: &Path) -> Option<Stored> {
        std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }
}

impl RoutineStore for FileRoutineStore {
    fn load(&self, owner: &Principal, project: &Path) -> RoutineBook {
        Self::read(&self.path(owner, project)).map(|stored| stored.book).unwrap_or_default()
    }

    fn save(&self, owner: &Principal, project: &Path, book: &RoutineBook) -> std::io::Result<()> {
        let path = self.path(owner, project);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let stored = Stored { owner: owner.clone(), project: project.to_path_buf(), book: book.clone() };
        let json = serde_json::to_vec_pretty(&stored).map_err(std::io::Error::other)?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json)?;
        std::fs::rename(partial, path)
    }

    fn all(&self) -> Vec<(Principal, PathBuf)> {
        let Ok(owners) = std::fs::read_dir(&self.root) else { return Vec::new() };
        owners
            .flatten()
            .filter_map(|owner| std::fs::read_dir(owner.path()).ok())
            .flat_map(|files| files.flatten())
            .filter(|file| file.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|file| Self::read(&file.path()))
            .map(|stored| (stored.owner, stored.project))
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct InMemoryRoutineStore {
    books: Mutex<HashMap<(Principal, PathBuf), RoutineBook>>,
}

impl RoutineStore for InMemoryRoutineStore {
    fn load(&self, owner: &Principal, project: &Path) -> RoutineBook {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.get(&(owner.clone(), project.to_path_buf())).cloned().unwrap_or_default()
    }

    fn save(&self, owner: &Principal, project: &Path, book: &RoutineBook) -> std::io::Result<()> {
        let mut books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.insert((owner.clone(), project.to_path_buf()), book.clone());
        Ok(())
    }

    fn all(&self) -> Vec<(Principal, PathBuf)> {
        let books = self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        books.keys().cloned().collect()
    }
}
