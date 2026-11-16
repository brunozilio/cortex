use std::path::Path;

use domain::memory::scorecard::RunOutcome;
use domain::{Principal, PromptRequest};

use super::{Memories, team};

impl Memories {
    pub(super) fn holds_out(&self, owner: &Principal, project: &Path) -> bool {
        let mut held = false;
        let mut books = self.lock();
        match self.store.update(owner, project, &mut |book| held = book.scorecard.next_held_out()) {
            Ok(changed) => *self.book(&mut books, owner, project) = changed,
            Err(error) => tracing::warn!(%error, "could not count the request for the memory scorecard"),
        }
        held
    }

    pub fn score(&self, owner: &Principal, project: &Path) -> String {
        let mut books = self.lock();
        self.book(&mut books, owner, project).scorecard.report()
    }

    pub(super) fn keep_score(&self, owner: &Principal, request: &PromptRequest, held: bool, outcome: &RunOutcome) {
        let features = team::features(&request.cwd, self.home(owner).as_deref());
        if !features.on(domain::features::MEMORY_SCORECARD) {
            return;
        }
        let mut books = self.lock();
        match self.store.update(owner, &request.cwd, &mut |book| book.scorecard.record(held, outcome)) {
            Ok(changed) => *self.book(&mut books, owner, &request.cwd) = changed,
            Err(error) => tracing::warn!(%error, "could not save the memory scorecard"),
        }
    }
}
