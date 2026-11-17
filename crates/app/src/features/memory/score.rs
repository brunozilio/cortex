use std::path::Path;

use domain::PromptRequest;
use domain::memory::scorecard::RunOutcome;

use super::{Memories, keep, team};

impl Memories {
    pub(super) fn holds_out(&self, project: &Path) -> bool {
        let mut held = false;
        let mut books = self.lock();
        match self.store.update(project, &mut |book| held = book.scorecard.next_held_out()) {
            Ok(changed) => {
                keep(&mut books, project, changed);
            }
            Err(error) => tracing::warn!(%error, "could not count the request for the memory scorecard"),
        }
        held
    }

    pub fn score(&self, project: &Path) -> String {
        let mut books = self.lock();
        self.book(&mut books, project).scorecard.report()
    }

    pub(super) fn keep_score(&self, request: &PromptRequest, held: bool, outcome: &RunOutcome) {
        let features = team::features(&self.gate(&request.session.cwd));
        if !features.on(domain::features::MEMORY_SCORECARD) {
            return;
        }
        let mut books = self.lock();
        match self.store.update(&request.session.cwd, &mut |book| book.scorecard.record(held, outcome)) {
            Ok(changed) => {
                keep(&mut books, &request.session.cwd, changed);
            }
            Err(error) => tracing::warn!(%error, "could not save the memory scorecard"),
        }
    }
}
