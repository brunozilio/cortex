use std::path::Path;

use domain::{MemoryBook, MemoryBrief, ModelOption, PromptRequest, RunId};
use platform::sync::Locked;
use protocol::features::memory::MemoryListing;

use super::{Memories, edits_file, listing};

impl Memories {
    pub fn optimization(&self, project: &Path, model: ModelOption) -> PromptRequest {
        let memories = {
            let mut books = self.lock();
            self.book(&mut books, project).memories.clone()
        };
        let prompt = domain::memory::optimize_prompt(&memories);
        PromptRequest {
            memory: Some(MemoryBrief { optimizing: true, ..MemoryBrief::default() }),
            ..PromptRequest::plain(project.to_path_buf(), prompt, model)
        }
    }

    pub fn undo(&self, project: &Path) -> Result<MemoryListing, String> {
        let key = project.to_path_buf();
        let point = self.undo_points().remove(&key);
        let Some(book) = point else { return Err("nothing to undo in the memory".into()) };
        self.store.save(project, &book).map_err(|error| format!("could not save memory: {error}"))?;
        let answer = listing(project, &book);
        self.lock().insert(key, book);
        Ok(answer)
    }

    pub(super) fn brief_optimization(&self, run: RunId, request: &PromptRequest) -> PromptRequest {
        let book = {
            let mut books = self.lock();
            self.book(&mut books, &request.cwd).clone()
        };
        self.set_undo_point(&request.cwd, Some(book));
        let brief = MemoryBrief { output: edits_file(run), optimizing: true, ..MemoryBrief::default() };
        PromptRequest { memory: Some(brief), ..request.clone() }
    }

    pub(super) fn set_undo_point(&self, project: &Path, book: Option<MemoryBook>) {
        let key = project.to_path_buf();
        let mut points = self.undo_points();
        match book {
            Some(book) => points.insert(key, book),
            None => points.remove(&key),
        };
    }

    fn undo_points(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<super::BookKey, MemoryBook>> {
        self.before_optimizing.locked()
    }
}
