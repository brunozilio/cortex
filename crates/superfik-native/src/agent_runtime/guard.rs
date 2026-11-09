//! Retained worktree cleanup when a child agent exits.

use super::*;

#[derive(Debug)]
pub struct Guard<'a> {
    pub setup: Setup,
    workspace: &'a Workspace,
    finished: bool,
    retained: bool,
}
impl<'a> Guard<'a> {
    pub fn new(setup: Setup, workspace: &'a Workspace) -> Self {
        Self { setup, workspace, finished: false, retained: false }
    }
    pub fn retain(&mut self) {
        self.retained = true;
    }
    pub fn finish(&mut self) -> Result<Option<PathBuf>, String> {
        self.finished = true;
        if self.retained {
            return Ok(Some(self.setup.cwd.clone()));
        }
        let cleanup_workspace = self.workspace.fork(std::sync::Arc::default());
        cleanup(&self.setup, &cleanup_workspace)
    }
}
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}
