//! The memory optimizer: one run in which an agent tidies a project's
//! memory (`superfik_core::memory::optimize_prompt`). Its edits apply like
//! any run's, and the book as it was before can be put back with
//! [`Memories::undo`] until the next edit.

use std::path::Path;

use superfik_core::{MemoryBook, MemoryBrief, ModelOption, Principal, PromptRequest, RunId};
use superfik_protocol::ServerEvent;

use super::{Memories, edits_file, memory_event};

impl Memories {
    /// The request for a run that tidies the memory of `project`.
    pub fn optimization(&self, owner: &Principal, project: &Path, model: ModelOption) -> PromptRequest {
        let memories = {
            let mut books = self.lock();
            self.book(&mut books, owner, project).memories.clone()
        };
        let prompt = superfik_core::memory::optimize_prompt(&memories);
        PromptRequest {
            memory: Some(MemoryBrief { optimizing: true, ..MemoryBrief::default() }),
            ..PromptRequest::plain(project.to_path_buf(), prompt, model)
        }
    }

    /// Puts back the book as it was before its last optimization.
    pub fn undo(&self, owner: &Principal, project: &Path) -> ServerEvent {
        let key = (owner.clone(), project.to_path_buf());
        let point = self.undo_points().remove(&key);
        let Some(book) = point else { return ServerEvent::error("nothing to undo in the memory") };
        if let Err(error) = self.store.save(owner, project, &book) {
            return ServerEvent::error(format!("could not save memory: {error}"));
        }
        let event = memory_event(project, &book);
        self.lock().insert(key, book);
        event
    }

    /// An optimizing run recalls nothing and gets no search tool; the book
    /// as it is now becomes the undo point.
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
