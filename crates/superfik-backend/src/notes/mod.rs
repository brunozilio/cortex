//! Project notes: where they live and the one place they change.
//!
//! A [`NoteStore`] keeps one [`NoteBook`] per owner and project. [`Notes`]
//! caches the books it has read and serialises every change, whether it
//! comes from the user (`EditNotes`) or from an agent's notes tool.

pub mod mcp;
mod store;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub use store::{FileNoteStore, InMemoryNoteStore, NoteStore};
use superfik_core::note::{NoteBook, NoteEdit, Stamp};
use superfik_core::{Principal, RunId};
use superfik_protocol::ServerEvent;

type BookKey = (Principal, PathBuf);

#[derive(Clone)]
pub struct Notes {
    store: Arc<dyn NoteStore>,
    books: Arc<Mutex<HashMap<BookKey, NoteBook>>>,
}

impl std::fmt::Debug for Notes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notes").finish_non_exhaustive()
    }
}

impl Notes {
    pub fn new(store: Arc<dyn NoteStore>) -> Self {
        Self { store, books: Arc::default() }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryNoteStore::default()))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<BookKey, NoteBook>> {
        self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn book(&self, owner: &Principal, project: &Path) -> NoteBook {
        let mut books = self.lock();
        books.entry((owner.clone(), project.to_path_buf())).or_insert_with(|| self.store.load(owner, project)).clone()
    }

    pub fn load(&self, owner: &Principal, project: &Path) -> ServerEvent {
        let book = self.book(owner, project);
        ServerEvent::Notes {
            path: project.to_path_buf(),
            notes: book.notes,
            by: None,
            created: vec![],
            refused: vec![],
        }
    }

    pub fn edit(
        &self,
        owner: &Principal,
        project: &Path,
        edits: &[NoteEdit],
        by: Stamp,
        run: Option<RunId>,
    ) -> ServerEvent {
        let mut books = self.lock();
        let key = (owner.clone(), project.to_path_buf());
        let book = books.entry(key).or_insert_with(|| self.store.load(owner, project));
        let mut changed = book.clone();
        let outcome = changed.apply(edits, by);
        if (!outcome.created.is_empty() || !outcome.changed.is_empty())
            && let Err(error) = self.store.save(owner, project, &changed)
        {
            tracing::warn!(%error, "could not save notes");
            return ServerEvent::error(format!("could not save notes: {error}"));
        }
        *book = changed;
        ServerEvent::Notes {
            path: project.to_path_buf(),
            notes: book.notes.clone(),
            by: run,
            created: outcome.created,
            refused: outcome.refused,
        }
    }
}

#[cfg(test)]
mod tests;
