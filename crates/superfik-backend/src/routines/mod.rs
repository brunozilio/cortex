//! Routines (ADR 0012): where they live, what is due, and how each firing
//! is recorded.
//!
//! A [`RoutineStore`] keeps one [`RoutineBook`] per owner and project.
//! [`Routines`] caches the books, serialises every change and answers with
//! `ServerEvent::Routines`. The handler asks it what is due on every tick
//! (`CommandHandler::fire_due_routines`) and tells it how each firing ended
//! ([`Trace`], carried by the run's sink).

mod firing;
mod store;
mod webhooks;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub use firing::{Firing, Trace, place, request};
pub use store::{FileRoutineStore, InMemoryRoutineStore, RoutineStore};
use superfik_core::routine::{RoutineBook, RoutineEdit, RoutineId, WebhookId};
use superfik_core::{Principal, RunId};
use superfik_protocol::ServerEvent;
pub use webhooks::{Refusal, Webhooks};

type BookKey = (Principal, PathBuf);

#[derive(Clone)]
pub struct Routines {
    store: Arc<dyn RoutineStore>,
    books: Arc<Mutex<HashMap<BookKey, RoutineBook>>>,
}

impl std::fmt::Debug for Routines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Routines").finish_non_exhaustive()
    }
}

impl Routines {
    pub fn new(store: Arc<dyn RoutineStore>) -> Self {
        Self { store, books: Arc::default() }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryRoutineStore::default()))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<BookKey, RoutineBook>> {
        self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn wake(&self, now: i64) {
        for (owner, project) in self.store.all() {
            self.change(&owner, &project, |book| book.settle(now));
        }
    }

    pub fn load(&self, owner: &Principal, project: &Path) -> ServerEvent {
        let book = {
            let mut books = self.lock();
            books
                .entry((owner.clone(), project.to_path_buf()))
                .or_insert_with(|| self.store.load(owner, project))
                .clone()
        };
        event(project, book)
    }

    pub fn edit(&self, owner: &Principal, project: &Path, edits: &[RoutineEdit], now: i64) -> ServerEvent {
        match self.try_change(owner, project, |book| {
            let outcome = book.apply(edits, now);
            !outcome.created.is_empty() || !outcome.changed.is_empty()
        }) {
            Ok(book) => event(project, book),
            Err(error) => ServerEvent::error(format!("could not save routines: {error}")),
        }
    }

    pub fn due(&self, now: i64) -> Vec<Firing> {
        let books = self.lock();
        let mut due = Vec::new();
        for ((owner, project), book) in books.iter() {
            for id in book.due(now) {
                if let Some(routine) = book.get(id) {
                    due.push(Firing { owner: owner.clone(), project: project.clone(), routine: routine.clone() });
                }
            }
        }
        due
    }

    pub fn listening(&self, owner: &Principal, connection: superfik_core::ConnectionId, exchange: &str) -> Vec<Firing> {
        let books = self.lock();
        let mut listening = Vec::new();
        for ((who, project), book) in books.iter().filter(|((who, _), _)| who == owner) {
            for id in book.listening(connection, exchange) {
                if let Some(routine) = book.get(id) {
                    listening.push(Firing { owner: who.clone(), project: project.clone(), routine: routine.clone() });
                }
            }
        }
        listening
    }

    pub fn hooked(&self, hook: WebhookId) -> Option<Firing> {
        let books = self.lock();
        books.iter().find_map(|((owner, project), book)| {
            let routine = book.get(book.hooked(hook)?)?;
            Some(Firing { owner: owner.clone(), project: project.clone(), routine: routine.clone() })
        })
    }

    pub fn webhooks(&self, owner: &Principal, project: &Path) -> Vec<(RoutineId, WebhookId)> {
        let books = self.lock();
        books
            .get(&(owner.clone(), project.to_path_buf()))
            .map(|book| book.routines.iter().filter_map(|r| Some((r.id, r.webhook?))).collect())
            .unwrap_or_default()
    }

    pub fn hooked_start(&self, firing: &Firing, run: RunId, branch: Option<String>, now: i64) -> Option<ServerEvent> {
        self.change(&firing.owner, &firing.project, |book| book.start_hooked(firing.routine.id, run, branch, now))
    }

    pub fn answering(
        &self,
        firing: &Firing,
        run: RunId,
        exchange: &str,
        branch: Option<String>,
        now: i64,
    ) -> Option<ServerEvent> {
        self.change(&firing.owner, &firing.project, |book| {
            book.start_answer(firing.routine.id, run, exchange, branch, now)
        })
    }

    pub fn started(&self, firing: &Firing, run: RunId, branch: Option<String>, now: i64) -> Option<ServerEvent> {
        self.change(&firing.owner, &firing.project, |book| book.start(firing.routine.id, run, branch, now))
    }

    fn change(
        &self,
        owner: &Principal,
        project: &Path,
        rule: impl FnOnce(&mut RoutineBook) -> bool,
    ) -> Option<ServerEvent> {
        let mut changed = false;
        let result = self.try_change(owner, project, |book| {
            changed = rule(book);
            changed
        });
        match result {
            Ok(book) if changed => Some(event(project, book)),
            Ok(_) => None,
            Err(error) => {
                tracing::warn!(%error, "could not save routines");
                None
            }
        }
    }

    fn try_change(
        &self,
        owner: &Principal,
        project: &Path,
        rule: impl FnOnce(&mut RoutineBook) -> bool,
    ) -> std::io::Result<RoutineBook> {
        let mut books = self.lock();
        let book =
            books.entry((owner.clone(), project.to_path_buf())).or_insert_with(|| self.store.load(owner, project));
        let mut next = book.clone();
        if rule(&mut next) {
            self.store.save(owner, project, &next)?;
            *book = next;
        }
        Ok(book.clone())
    }
}

fn event(project: &Path, book: RoutineBook) -> ServerEvent {
    ServerEvent::Routines { path: project.to_path_buf(), book }
}

#[cfg(test)]
mod tests;
