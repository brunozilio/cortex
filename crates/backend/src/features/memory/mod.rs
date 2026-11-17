mod check;
mod command;
pub mod mcp;
mod optimize;
pub mod run;
mod score;
mod seed;
mod store;
mod team;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::memory::scorecard::RunOutcome;
use domain::memory::{MemorySearches, SearchAnswer, recall, search};
use domain::{
    Author, Memory, MemoryBook, MemoryBrief, MemoryEdit, MemoryId, PromptRequest, Provenance, RecallBudget, RunId,
};
use platform::sync::Locked;
use protocol::features::memory::{MemoryApplied, MemoryListing};
#[cfg(test)]
use store::{FileMemoryStore, InMemoryStore};
use store::{MemoryStore, memory_store_in};

use crate::services::LocalServices;

pub use command::handle;

type BookKey = PathBuf;

#[derive(Clone)]
pub struct Memories {
    store: Arc<dyn MemoryStore>,
    books: Arc<Mutex<HashMap<BookKey, MemoryBook>>>,
    before_optimizing: Arc<Mutex<HashMap<BookKey, MemoryBook>>>,
    homes: Arc<dyn crate::Homes>,
}

impl std::fmt::Debug for Memories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Memories").finish_non_exhaustive()
    }
}

pub fn forget(_client: u64) {}

pub fn of(services: &LocalServices) -> Memories {
    static ALL: LazyLock<Mutex<HashMap<Option<PathBuf>, Memories>>> = LazyLock::new(Mutex::default);
    let mut all = ALL.locked();
    let folder = services.root.memory();
    all.entry(folder.clone())
        .or_insert_with(|| Memories::new(memory_store_in(folder)).with_homes(Arc::clone(&services.homes)))
        .clone()
}

impl Memories {
    pub fn new(store: Arc<dyn MemoryStore>) -> Self {
        Self {
            store,
            books: Arc::default(),
            before_optimizing: Arc::default(),
            homes: Arc::new(crate::OwnerHome::none()),
        }
    }

    pub fn with_homes(mut self, homes: Arc<dyn crate::Homes>) -> Self {
        self.homes = homes;
        self
    }

    fn home(&self) -> Option<PathBuf> {
        self.homes.home()
    }

    fn book<'a>(&self, books: &'a mut MutexGuard<HashMap<BookKey, MemoryBook>>, project: &Path) -> &'a mut MemoryBook {
        books.entry(project.to_path_buf()).or_insert_with(|| self.store.load(project))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<BookKey, MemoryBook>> {
        self.books.locked()
    }

    pub fn load(&self, project: &Path) -> MemoryListing {
        let mut books = self.lock();
        listing(project, self.book(&mut books, project))
    }

    pub fn edit(&self, project: &Path, edits: &[MemoryEdit], by: Provenance) -> Result<MemoryListing, String> {
        self.set_undo_point(project, None);
        self.apply(project, edits, by)
    }

    fn apply(&self, project: &Path, edits: &[MemoryEdit], by: Provenance) -> Result<MemoryListing, String> {
        let mut books = self.lock();
        let changed = self.store.update(project, &mut |book| {
            book.apply(edits, &by);
        });
        let changed = match changed {
            Ok(changed) => changed,
            Err(error) => {
                tracing::warn!(%error, "could not save memory");
                return Err(format!("could not save memory: {error}"));
            }
        };
        let book = self.book(&mut books, project);
        *book = changed;
        Ok(listing(project, book))
    }

    pub fn search(&self, project: &Path, query: &str) -> Vec<(MemoryId, f32)> {
        let mut books = self.lock();
        let book = self.book(&mut books, project);
        let scores = search(query, &book.memories);
        let mut hits: Vec<(MemoryId, f32)> =
            book.memories.iter().zip(scores).filter(|(_, s)| *s > 0.0).map(|(m, s)| (m.id, s)).collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        hits
    }

    pub fn find(&self, project: &Path, query: &str, limit: usize, searches: &mut MemorySearches) -> SearchAnswer {
        let shared = team::load(project, self.home().as_deref());
        let mut books = self.lock();
        let book = self.book(&mut books, project);
        let known: Vec<Memory> = book.memories.iter().cloned().chain(shared).collect();
        let scores = search(query, &known);
        let mut order: Vec<usize> = (0..known.len()).filter(|&i| scores[i] > 0.0).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
        let ranked: Vec<Memory> = order.into_iter().map(|i| known[i].clone()).collect();
        let answer = searches.answer(ranked, limit);
        if let SearchAnswer::Found(found) = &answer {
            let personal: Vec<Memory> =
                found.iter().filter(|memory| !domain::memory::team::is_team(memory.id)).cloned().collect();
            self.mark_used(project, book, &personal);
        }
        answer
    }

    pub fn shared(&self, project: &Path, id: MemoryId) -> Result<PathBuf, String> {
        let memory = {
            let mut books = self.lock();
            let book = self.book(&mut books, project);
            book.memories.iter().find(|memory| memory.id == id).cloned()
        };
        team::share(project, self.home().as_deref(), &memory.ok_or("there is no such memory")?)
    }

    fn mark_used(&self, project: &Path, book: &mut MemoryBook, given: &[Memory]) {
        if given.is_empty() {
            return;
        }
        let ids: Vec<MemoryId> = given.iter().map(|m| m.id).collect();
        let at = now();
        match self.store.update(project, &mut |stored| stored.mark_used(&ids, at)) {
            Ok(stored) => *book = stored,
            Err(error) => {
                book.mark_used(&ids, at);
                tracing::warn!(%error, "could not save memory use");
            }
        }
    }

    pub fn brief(&self, run: RunId, request: &PromptRequest, tool: bool) -> PromptRequest {
        let Some(brief) = &request.memory else { return request.clone() };
        if brief.optimizing {
            return self.brief_optimization(run, request);
        }
        let budget = if tool { RecallBudget::WITH_TOOL } else { RecallBudget::default() };
        let features = team::features(&request.cwd, self.home().as_deref());
        let anchors = features.on(domain::features::MEMORY_ANCHORS);
        let shared = team::load_if(&request.cwd, &features);
        let structured = features.on(domain::features::STRUCTURED_LEARNINGS);
        if features.on(domain::features::MEMORY_SCORECARD) && self.holds_out(&request.cwd) {
            let brief = MemoryBrief {
                output: edits_file(run),
                origin: brief.origin.clone(),
                anchors,
                structured,
                held_out: true,
                ..MemoryBrief::default()
            };
            return PromptRequest { memory: Some(brief), ..request.clone() };
        }
        let (recalled, tool, anchored) = {
            let mut books = self.lock();
            let book = self.book(&mut books, &request.cwd);
            let known: Vec<Memory> = book.memories.iter().cloned().chain(shared).collect();
            let recalled = recall(&request.prompt, &known, budget, now());
            let personal: Vec<Memory> =
                recalled.iter().filter(|memory| !domain::memory::team::is_team(memory.id)).cloned().collect();
            self.mark_used(&request.cwd, book, &personal);
            let tool = tool && book.holds_more_than(&recalled);
            let anchored: Vec<Memory> = if anchors {
                let unseen: Vec<Memory> =
                    known.into_iter().filter(|memory| !recalled.iter().any(|r| r.id == memory.id)).collect();
                domain::memory::anchors::anchored(&unseen)
            } else {
                Vec::new()
            };
            (recalled, tool, anchored)
        };
        let brief = MemoryBrief {
            recalled,
            output: edits_file(run),
            origin: brief.origin.clone(),
            tool,
            optimizing: false,
            anchored,
            anchors,
            structured,
            held_out: false,
        };
        PromptRequest { memory: Some(brief), ..request.clone() }
    }

    pub fn finish(&self, request: &PromptRequest, outcome: &RunOutcome) -> Option<Result<MemoryApplied, String>> {
        let brief = request.memory.as_ref()?;
        if !brief.optimizing {
            self.keep_score(request, brief.held_out, outcome);
        }
        let completed = outcome.completed;
        let json = completed.then(|| std::fs::read_to_string(&brief.output).ok()).flatten();
        let _ = std::fs::remove_file(&brief.output);
        let edits = json.and_then(|json| domain::memory::parse_memory_edits(&json)).unwrap_or_default();
        if edits.is_empty() {
            if brief.optimizing {
                self.set_undo_point(&request.cwd, None);
            }
            return None;
        }
        let by = Provenance { author: Author::Agent, now: now(), origin: brief.origin.clone() };
        let before = self.load(&request.cwd).memories;
        let applied =
            if brief.optimizing { self.apply(&request.cwd, &edits, by) } else { self.edit(&request.cwd, &edits, by) };
        Some(applied.map(|listing| applied_since(&before, listing, brief.optimizing)))
    }
}

fn applied_since(before: &[Memory], listing: MemoryListing, optimizing: bool) -> MemoryApplied {
    let was = |id: MemoryId| before.iter().find(|memory| memory.id == id);
    let reworded =
        |memory: &&Memory| was(memory.id).is_some_and(|old| (&old.text, old.kind) != (&memory.text, memory.kind));
    MemoryApplied {
        optimizing,
        added: listing.memories.iter().filter(|memory| was(memory.id).is_none()).count(),
        changed: listing.memories.iter().filter(reworded).count(),
        forgotten: before.iter().filter(|old| !listing.memories.iter().any(|memory| memory.id == old.id)).count(),
        listing,
    }
}

fn listing(project: &Path, book: &MemoryBook) -> MemoryListing {
    MemoryListing { path: project.to_path_buf(), memories: book.memories.clone(), forgotten: book.forgotten.clone() }
}

fn edits_file(run: RunId) -> PathBuf {
    let folder = std::env::temp_dir().join(concat!(brand::command!(), "-memory"));
    if let Err(error) = std::fs::create_dir_all(&folder) {
        tracing::warn!(%error, "could not create the memory edits folder");
    }
    folder.join(format!("{run}.json"))
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod anchors_tests;
#[cfg(test)]
mod tests;
