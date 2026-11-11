use std::path::Path;

use domain::{MemoryBook, MemoryBrief, ModelOption, Principal, PromptRequest, RunId};
use serde_json::Value;

use super::{Memories, edits_file, listing};

impl Memories {
    pub fn optimization(&self, owner: &Principal, project: &Path, model: ModelOption) -> PromptRequest {
        let memories = {
            let mut books = self.lock();
            self.book(&mut books, owner, project).memories.clone()
        };
        let prompt = domain::memory::optimize_prompt(&memories);
        PromptRequest {
            memory: Some(MemoryBrief { optimizing: true, ..MemoryBrief::default() }),
            ..PromptRequest::plain(project.to_path_buf(), prompt, model)
        }
    }

    pub fn undo(&self, owner: &Principal, project: &Path) -> Result<Value, String> {
        let key = (owner.clone(), project.to_path_buf());
        let point = self.undo_points().remove(&key);
        let Some(book) = point else { return Err("nothing to undo in the memory".into()) };
        self.store.save(owner, project, &book).map_err(|error| format!("could not save memory: {error}"))?;
        let answer = listing(project, &book);
        self.lock().insert(key, book);
        Ok(answer)
    }

    pub(super) fn brief_optimization(&self, owner: &Principal, run: RunId, request: &PromptRequest) -> PromptRequest {
        let book = {
            let mut books = self.lock();
            self.book(&mut books, owner, &request.cwd).clone()
        };
        self.set_undo_point(owner, &request.cwd, Some(book));
        let brief = MemoryBrief { output: edits_file(run), optimizing: true, ..MemoryBrief::default() };
        PromptRequest { memory: Some(brief), ..request.clone() }
    }

    pub(super) fn set_undo_point(&self, owner: &Principal, project: &Path, book: Option<MemoryBook>) {
        let key = (owner.clone(), project.to_path_buf());
        let mut points = self.undo_points();
        match book {
            Some(book) => points.insert(key, book),
            None => points.remove(&key),
        };
    }

    fn undo_points(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<super::BookKey, MemoryBook>> {
        self.before_optimizing.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
