//! Project memories: where they live, how they are searched, and the one
//! place they change.
//!
//! A [`MemoryStore`] keeps one [`MemoryBook`] per owner and project.
//! [`Memories`] caches the books it has read, serialises every change
//! (runs finish on their own threads), keeps each memory's embedding
//! vector up to date and answers searches with the hybrid ranking of
//! `domain::memory::search`.

mod embedder;
pub mod mcp;
mod optimize;
mod store;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

pub use embedder::{Embedder, ModelChoice, NoEmbedder, Purpose, local};
pub use store::{
    FileMemoryStore, InMemoryStore, MemoryStore, app_data_dir, owner_key, project_key, system_memory_store,
};
use domain::memory::{Semantic, recall, search};
use domain::{
    Author, Memory, MemoryBook, MemoryBrief, MemoryEdit, MemoryId, Principal, PromptRequest, Provenance, RecallBudget,
    RunId,
};
use protocol::ServerEvent;

type BookKey = (Principal, PathBuf);

#[derive(Clone)]
pub struct Memories {
    store: Arc<dyn MemoryStore>,
    embedder: Arc<dyn Embedder>,
    books: Arc<Mutex<HashMap<BookKey, MemoryBook>>>,
    /// Each book as it was before its last optimization, until another edit.
    before_optimizing: Arc<Mutex<HashMap<BookKey, MemoryBook>>>,
}

impl std::fmt::Debug for Memories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Memories").field("model", &self.embedder.model()).finish_non_exhaustive()
    }
}

impl Memories {
    pub fn new(store: Arc<dyn MemoryStore>, embedder: Arc<dyn Embedder>) -> Self {
        Self { store, embedder, books: Arc::default(), before_optimizing: Arc::default() }
    }

    pub fn with_store(&self, store: Arc<dyn MemoryStore>) -> Self {
        Self::new(store, Arc::clone(&self.embedder))
    }

    pub fn plain(store: Arc<dyn MemoryStore>) -> Self {
        Self::new(store, Arc::new(NoEmbedder))
    }

    fn book<'a>(
        &self,
        books: &'a mut MutexGuard<HashMap<BookKey, MemoryBook>>,
        owner: &Principal,
        project: &Path,
    ) -> &'a mut MemoryBook {
        let key = (owner.clone(), project.to_path_buf());
        let book = books.entry(key).or_insert_with(|| self.store.load(owner, project));
        if self.embed_missing(book) && self.store.save(owner, project, book).is_err() {
            tracing::warn!("could not save memory vectors");
        }
        book
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<BookKey, MemoryBook>> {
        self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn embed_missing(&self, book: &mut MemoryBook) -> bool {
        let model = self.embedder.model();
        if model == NoEmbedder.model() {
            return false;
        }
        let missing = book.unembedded(model);
        if missing.is_empty() {
            return false;
        }
        let texts: Vec<String> = missing.iter().map(|(_, text)| text.clone()).collect();
        let Some(vectors) = self.embedder.embed(&texts, Purpose::Passage) else { return false };
        for ((id, _), vector) in missing.into_iter().zip(vectors) {
            book.vectors.insert(id, vector);
        }
        true
    }

    pub fn load(&self, owner: &Principal, project: &Path) -> ServerEvent {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        memory_event(project, book)
    }

    /// Applies edits from the user or a run; the last optimization can no
    /// longer be undone.
    pub fn edit(&self, owner: &Principal, project: &Path, edits: &[MemoryEdit], by: Provenance) -> ServerEvent {
        self.set_undo_point(owner, project, None);
        self.apply(owner, project, edits, by)
    }

    fn apply(&self, owner: &Principal, project: &Path, edits: &[MemoryEdit], by: Provenance) -> ServerEvent {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        let mut changed = book.clone();
        let since = changed.next_id();
        changed.apply(edits, &by);
        self.embed_missing(&mut changed);
        changed.merge_near_duplicates(since, by.now);
        if let Err(error) = self.store.save(owner, project, &changed) {
            tracing::warn!(%error, "could not save memory");
            return ServerEvent::error(format!("could not save memory: {error}"));
        }
        *book = changed;
        memory_event(project, book)
    }

    pub fn search(&self, owner: &Principal, project: &Path, query: &str) -> Vec<(MemoryId, f32)> {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        let scores = self.scores(book, query);
        let mut hits: Vec<(MemoryId, f32)> =
            book.memories.iter().zip(scores).filter(|(_, s)| *s > 0.0).map(|(m, s)| (m.id, s)).collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        hits
    }

    pub fn find(&self, owner: &Principal, project: &Path, query: &str, limit: usize) -> Vec<Memory> {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        let scores = self.scores(book, query);
        let mut order: Vec<usize> = (0..book.memories.len()).filter(|&i| scores[i] > 0.0).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
        let found: Vec<Memory> = order.into_iter().take(limit).map(|i| book.memories[i].clone()).collect();
        self.mark_used(owner, project, book, &found);
        found
    }

    /// What reaches an agent counts as used, which slows an episode's
    /// fading (`domain::memory::retention`).
    fn mark_used(&self, owner: &Principal, project: &Path, book: &mut MemoryBook, given: &[Memory]) {
        if given.is_empty() {
            return;
        }
        let ids: Vec<MemoryId> = given.iter().map(|m| m.id).collect();
        book.mark_used(&ids, now());
        if let Err(error) = self.store.save(owner, project, book) {
            tracing::warn!(%error, "could not save memory use");
        }
    }

    pub fn search_event(&self, owner: &Principal, project: &Path, query: &str) -> ServerEvent {
        let scores = self.search(owner, project, query);
        ServerEvent::MemorySearch { path: project.to_path_buf(), query: query.to_owned(), scores }
    }

    fn scores(&self, book: &MemoryBook, query: &str) -> Vec<f32> {
        let vector = self.query_vector(book, query);
        let aligned = book.aligned_vectors();
        let semantic = vector.as_deref().map(|query| Semantic { query, memories: &aligned });
        search(query, &book.memories, semantic)
    }

    fn query_vector(&self, book: &MemoryBook, query: &str) -> Option<Vec<f32>> {
        if query.trim().is_empty() || book.model.as_deref() != Some(self.embedder.model()) {
            return None;
        }
        self.embedder.embed(&[query.to_owned()], Purpose::Query)?.into_iter().next()
    }

    pub fn brief(&self, owner: &Principal, run: RunId, request: &PromptRequest, tool: bool) -> PromptRequest {
        let Some(brief) = &request.memory else { return request.clone() };
        if brief.optimizing {
            return self.brief_optimization(owner, run, request);
        }
        let budget = if tool { RecallBudget::WITH_TOOL } else { RecallBudget::default() };
        let recalled = {
            let mut books = self.lock();
            let book = self.book(&mut books, owner, &request.cwd);
            let vector = self.query_vector(book, &request.prompt);
            let aligned = book.aligned_vectors();
            let semantic = vector.as_deref().map(|query| Semantic { query, memories: &aligned });
            let recalled = recall(&request.prompt, &book.memories, semantic, budget, now());
            self.mark_used(owner, &request.cwd, book, &recalled);
            recalled
        };
        let output = edits_file(run);
        let brief = MemoryBrief { recalled, output, origin: brief.origin.clone(), tool, optimizing: false };
        PromptRequest { memory: Some(brief), ..request.clone() }
    }

    pub fn finish(&self, owner: &Principal, request: &PromptRequest, completed: bool) -> Option<ServerEvent> {
        let brief = request.memory.as_ref()?;
        let json = completed.then(|| std::fs::read_to_string(&brief.output).ok()).flatten();
        let _ = std::fs::remove_file(&brief.output);
        let edits = json.and_then(|json| domain::memory::parse_memory_edits(&json)).unwrap_or_default();
        if edits.is_empty() {
            if brief.optimizing {
                self.set_undo_point(owner, &request.cwd, None);
            }
            return None;
        }
        let by = Provenance { author: Author::Agent, now: now(), origin: brief.origin.clone() };
        if brief.optimizing {
            Some(self.apply(owner, &request.cwd, &edits, by))
        } else {
            Some(self.edit(owner, &request.cwd, &edits, by))
        }
    }
}

fn memory_event(project: &Path, book: &MemoryBook) -> ServerEvent {
    ServerEvent::Memory {
        path: project.to_path_buf(),
        memories: book.memories.clone(),
        forgotten: book.forgotten.clone(),
    }
}

fn edits_file(run: RunId) -> PathBuf {
    let folder = std::env::temp_dir().join("superfik-memory");
    if let Err(error) = std::fs::create_dir_all(&folder) {
        tracing::warn!(%error, "could not create the memory edits folder");
    }
    folder.join(format!("{run}.json"))
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests;
