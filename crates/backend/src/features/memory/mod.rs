pub mod mcp;
mod optimize;
pub mod run;
mod store;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use domain::memory::{recall, search};
use domain::{
    Author, Memory, MemoryBook, MemoryBrief, MemoryEdit, MemoryId, ModelOption, Principal, PromptRequest, Provenance,
    RecallBudget, RunId,
};
use serde_json::{Value, json};
#[cfg(test)]
use store::{FileMemoryStore, InMemoryStore};
use store::{MemoryStore, memory_store_in};

use crate::feature::Context;
use crate::services::LocalServices;

pub const NAME: &str = "memory";

type BookKey = (Principal, PathBuf);

#[derive(Clone)]
pub struct Memories {
    store: Arc<dyn MemoryStore>,
    books: Arc<Mutex<HashMap<BookKey, MemoryBook>>>,
    before_optimizing: Arc<Mutex<HashMap<BookKey, MemoryBook>>>,
}

impl std::fmt::Debug for Memories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Memories").finish_non_exhaustive()
    }
}

pub fn handle(context: &Context<'_>, input: Value) -> Result<Value, String> {
    let memories = of(context.services);
    let owner = context.principal;
    let path: PathBuf = input["path"].as_str().map(PathBuf::from).ok_or("the memory feature needs a path")?;
    match input["op"].as_str().unwrap_or("list") {
        "list" => Ok(memories.load(owner, &path)),
        "search" => Ok(memories.search_listing(owner, &path, input["query"].as_str().unwrap_or_default())),
        "edit" => {
            let edits: Vec<MemoryEdit> = serde_json::from_value(input["edits"].clone()).map_err(|e| e.to_string())?;
            memories.edit(owner, &path, &edits, Provenance { author: Author::User, now: now(), origin: None })
        }
        "undo" => memories.undo(owner, &path),
        "optimize" => {
            let run: RunId = serde_json::from_value(input["run"].clone()).map_err(|e| e.to_string())?;
            let model: ModelOption = serde_json::from_value(input["model"].clone()).map_err(|e| e.to_string())?;
            (context.submit)(run, memories.optimization(owner, &path, model))?;
            Ok(json!({ "run": run }))
        }
        other => Err(format!("the memory feature has no {other:?}")),
    }
}

pub fn forget(_client: u64) {}

pub fn of(services: &LocalServices) -> Memories {
    static ALL: LazyLock<Mutex<HashMap<Option<PathBuf>, Memories>>> = LazyLock::new(Mutex::default);
    let mut all = ALL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    all.entry(services.data.clone()).or_insert_with(|| Memories::new(memory_store_in(services.data.clone()))).clone()
}

impl Memories {
    pub fn new(store: Arc<dyn MemoryStore>) -> Self {
        Self { store, books: Arc::default(), before_optimizing: Arc::default() }
    }

    fn book<'a>(
        &self,
        books: &'a mut MutexGuard<HashMap<BookKey, MemoryBook>>,
        owner: &Principal,
        project: &Path,
    ) -> &'a mut MemoryBook {
        let key = (owner.clone(), project.to_path_buf());
        books.entry(key).or_insert_with(|| self.store.load(owner, project))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<BookKey, MemoryBook>> {
        self.books.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn load(&self, owner: &Principal, project: &Path) -> Value {
        let mut books = self.lock();
        listing(project, self.book(&mut books, owner, project))
    }

    pub fn edit(
        &self,
        owner: &Principal,
        project: &Path,
        edits: &[MemoryEdit],
        by: Provenance,
    ) -> Result<Value, String> {
        self.set_undo_point(owner, project, None);
        self.apply(owner, project, edits, by)
    }

    fn apply(&self, owner: &Principal, project: &Path, edits: &[MemoryEdit], by: Provenance) -> Result<Value, String> {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        let mut changed = book.clone();
        let since = changed.next_id();
        changed.apply(edits, &by);
        changed.merge_near_duplicates(since, by.now);
        if let Err(error) = self.store.save(owner, project, &changed) {
            tracing::warn!(%error, "could not save memory");
            return Err(format!("could not save memory: {error}"));
        }
        *book = changed;
        Ok(listing(project, book))
    }

    pub fn search(&self, owner: &Principal, project: &Path, query: &str) -> Vec<(MemoryId, f32)> {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        let scores = search(query, &book.memories, None);
        let mut hits: Vec<(MemoryId, f32)> =
            book.memories.iter().zip(scores).filter(|(_, s)| *s > 0.0).map(|(m, s)| (m.id, s)).collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        hits
    }

    fn search_listing(&self, owner: &Principal, project: &Path, query: &str) -> Value {
        json!({ "path": project, "query": query, "scores": self.search(owner, project, query) })
    }

    pub fn find(&self, owner: &Principal, project: &Path, query: &str, limit: usize) -> Vec<Memory> {
        let mut books = self.lock();
        let book = self.book(&mut books, owner, project);
        let scores = search(query, &book.memories, None);
        let mut order: Vec<usize> = (0..book.memories.len()).filter(|&i| scores[i] > 0.0).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
        let found: Vec<Memory> = order.into_iter().take(limit).map(|i| book.memories[i].clone()).collect();
        self.mark_used(owner, project, book, &found);
        found
    }

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

    pub fn brief(&self, owner: &Principal, run: RunId, request: &PromptRequest, tool: bool) -> PromptRequest {
        let Some(brief) = &request.memory else { return request.clone() };
        if brief.optimizing {
            return self.brief_optimization(owner, run, request);
        }
        let budget = if tool { RecallBudget::WITH_TOOL } else { RecallBudget::default() };
        let recalled = {
            let mut books = self.lock();
            let book = self.book(&mut books, owner, &request.cwd);
            let recalled = recall(&request.prompt, &book.memories, None, budget, now());
            self.mark_used(owner, &request.cwd, book, &recalled);
            recalled
        };
        let output = edits_file(run);
        let brief = MemoryBrief { recalled, output, origin: brief.origin.clone(), tool, optimizing: false };
        PromptRequest { memory: Some(brief), ..request.clone() }
    }

    pub fn finish(&self, owner: &Principal, request: &PromptRequest, completed: bool) -> Option<Result<Value, String>> {
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

fn listing(project: &Path, book: &MemoryBook) -> Value {
    json!({ "path": project, "memories": book.memories, "forgotten": book.forgotten })
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
mod tests;
