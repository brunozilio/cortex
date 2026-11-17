use std::path::Path;

use crate::api::features::workspace::{WorkspaceOp, WorkspaceReply, WorkspaceRequest};
use domain::changes::{Changes, DiffScope};

use crate::client::features::unexpected;
use crate::{Client, Error};

impl Client {
    pub fn changes(&self, cwd: &Path, scope: DiffScope) -> Result<Changes, Error> {
        match self.workspace(cwd, WorkspaceOp::Changes { scope })? {
            WorkspaceReply::Changes(changes) => Ok(changes),
            _ => Err(unexpected()),
        }
    }

    pub fn project_files(&self, cwd: &Path) -> Result<Vec<String>, Error> {
        match self.workspace(cwd, WorkspaceOp::Files)? {
            WorkspaceReply::Files(files) => Ok(files),
            _ => Err(unexpected()),
        }
    }

    pub fn git_offered(&self, cwd: &Path) -> Result<bool, Error> {
        match self.workspace(cwd, WorkspaceOp::GitOffer)? {
            WorkspaceReply::GitOffer { offer } => Ok(offer),
            _ => Err(unexpected()),
        }
    }

    pub fn init_git(&self, cwd: &Path) -> Result<bool, Error> {
        match self.workspace(cwd, WorkspaceOp::GitInit)? {
            WorkspaceReply::GitInit { committed } => Ok(committed),
            _ => Err(unexpected()),
        }
    }

    pub fn with_mentioned_files(&self, cwd: &Path, prompt: &str) -> Result<String, Error> {
        match self.workspace(cwd, WorkspaceOp::Mentions { prompt: prompt.to_owned() })? {
            WorkspaceReply::Mentions(prompt) => Ok(prompt),
            _ => Err(unexpected()),
        }
    }

    fn workspace(&self, cwd: &Path, op: WorkspaceOp) -> Result<WorkspaceReply, Error> {
        self.ask_workspace(WorkspaceRequest { path: cwd.to_path_buf(), op })
    }
}
