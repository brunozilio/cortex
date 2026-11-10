//! Retained worktree cleanup when a child agent exits.

use super::*;

#[derive(Debug)]
pub struct Guard {
    pub setup: Setup,
    finished: bool,
    retained: bool,
}

impl Guard {
    pub fn new(setup: Setup) -> Self {
        Self { setup, finished: false, retained: false }
    }

    pub fn retain(&mut self) {
        self.retained = true;
    }

    pub fn finish(&mut self) -> Result<Option<PathBuf>, String> {
        self.finished = true;
        if self.retained {
            return Ok(Some(self.setup.cwd.clone()));
        }
        cleanup(&self.setup)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}
