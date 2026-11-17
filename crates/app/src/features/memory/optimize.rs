use std::path::Path;

use crate::api::features::memory::MemoryListing;
use domain::{MemoryBook, MemoryBrief, ModelOption, PromptRequest, RunId};

use super::{Memories, edits_file, keep, listing};

impl Memories {
    pub fn optimization(&self, project: &Path, model: ModelOption) -> PromptRequest {
        let memories = {
            let mut books = self.lock();
            self.book(&mut books, project).memories.clone()
        };
        let prompt = domain::memory::optimize_prompt(&memories);
        let mut request = PromptRequest::plain(project.to_path_buf(), prompt, model);
        request.host.memory = Some(MemoryBrief { optimizing: true, ..MemoryBrief::default() });
        request
    }

    pub fn undo(&self, project: &Path) -> Result<MemoryListing, String> {
        let Some(book) = self.store.undo_point(project) else { return Err("nothing to undo in the memory".into()) };
        let mut books = self.lock();
        let restored = self
            .store
            .update(project, &mut |kept| *kept = book.clone())
            .map_err(|error| format!("could not save memory: {error}"))?;
        self.set_undo_point(project, None);
        Ok(listing(project, keep(&mut books, project, restored)))
    }

    pub(super) fn brief_optimization(&self, run: RunId, request: &PromptRequest) -> PromptRequest {
        let book = {
            let mut books = self.lock();
            self.book(&mut books, &request.session.cwd).clone()
        };
        self.set_undo_point(&request.session.cwd, Some(book));
        let mut briefed = request.clone();
        briefed.host.memory = Some(MemoryBrief { output: edits_file(run), optimizing: true, ..MemoryBrief::default() });
        briefed
    }

    pub(super) fn set_undo_point(&self, project: &Path, book: Option<MemoryBook>) {
        if let Err(error) = self.store.set_undo_point(project, book.as_ref()) {
            tracing::warn!(%error, "could not keep the point the memory can be undone to");
        }
    }
}
