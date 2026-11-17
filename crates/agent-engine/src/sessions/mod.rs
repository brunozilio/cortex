#[cfg(any(test, feature = "test-support"))]
use std::collections::HashMap;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;

use domain::legacy::LEGACY_OWNER_TAG;
use domain::{PromptRequest, SessionId};
use sha2::{Digest, Sha256};

use crate::checkpoints::Checkpoint;
use crate::transcript::Message;

pub trait Sessions: Send + Sync {
    fn load(&self, id: &SessionId) -> Option<Vec<Message>>;
    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String>;
    fn load_worktree(
        &self,
        _request: &PromptRequest,
        _id: &SessionId,
    ) -> Result<Option<crate::agent_runtime::WorktreeState>, String> {
        Ok(None)
    }
    fn save_worktree(
        &self,
        _request: &PromptRequest,
        _id: &SessionId,
        _state: &crate::agent_runtime::WorktreeState,
    ) -> Result<(), String> {
        Err("session store does not support persistent worktrees".into())
    }
    fn forget_worktree(&self, _request: &PromptRequest, _id: &SessionId) -> Result<(), String> {
        Ok(())
    }
    fn load_for(&self, request: &PromptRequest, id: &SessionId) -> Option<Vec<Message>> {
        self.load(&scoped_id(request, id))
    }
    fn record(&self, _request: &PromptRequest, _id: &SessionId, _entry: &Entry) -> Result<(), String> {
        Ok(())
    }
    fn hold(&self, _request: &PromptRequest, _id: &SessionId) -> Result<Hold, String> {
        Ok(Hold::none())
    }
    fn length(&self, _request: &PromptRequest, _id: &SessionId) -> Result<u64, String> {
        Ok(0)
    }
    fn load_for_checked(&self, request: &PromptRequest, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        Ok(self.load_for(request, id))
    }
    fn save_for(&self, request: &PromptRequest, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.save(&scoped_id(request, id), history)
    }
    fn save_compacted(&self, request: &PromptRequest, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.save_for(request, id, history)
    }
    fn load_checkpoints(&self, _request: &PromptRequest, _id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        Ok(Vec::new())
    }
    fn save_checkpoints(&self, _request: &PromptRequest, _id: &SessionId, _all: &[Checkpoint]) -> Result<(), String> {
        Ok(())
    }
    fn load_snapshot(&self, _request: &PromptRequest, _id: &SessionId) -> Option<crate::prompt::Snapshot> {
        None
    }
    fn save_snapshot(
        &self,
        _request: &PromptRequest,
        _id: &SessionId,
        _snapshot: &crate::prompt::Snapshot,
    ) -> Result<(), String> {
        Ok(())
    }
}

fn scoped_id(request: &PromptRequest, id: &SessionId) -> SessionId {
    let cwd = request.cwd.canonicalize().unwrap_or_else(|_| request.cwd.clone());
    let scope = (LEGACY_OWNER_TAG, cwd.to_string_lossy(), id);
    let bytes = serde_json::to_vec(&scope).unwrap_or_else(|_| format!("{scope:?}").into_bytes());
    SessionId(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Default)]
pub struct InMemorySessions {
    kept: Mutex<HashMap<SessionId, Vec<Message>>>,
    worktrees: Mutex<HashMap<SessionId, crate::agent_runtime::WorktreeState>>,
    checkpoints: Mutex<HashMap<SessionId, Vec<Checkpoint>>>,
    snapshots: Mutex<HashMap<SessionId, crate::prompt::Snapshot>>,
}

#[cfg(any(test, feature = "test-support"))]
impl Sessions for InMemorySessions {
    fn load_snapshot(&self, request: &PromptRequest, id: &SessionId) -> Option<crate::prompt::Snapshot> {
        self.snapshots.lock().ok()?.get(&scoped_id(request, id)).cloned()
    }
    fn save_snapshot(
        &self,
        request: &PromptRequest,
        id: &SessionId,
        snapshot: &crate::prompt::Snapshot,
    ) -> Result<(), String> {
        self.snapshots.lock().map_err(|_| "snapshots unavailable")?.insert(scoped_id(request, id), snapshot.clone());
        Ok(())
    }
    fn load_checkpoints(&self, request: &PromptRequest, id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        let kept = self.checkpoints.lock().map_err(|_| "checkpoints unavailable")?;
        Ok(kept.get(&scoped_id(request, id)).cloned().unwrap_or_default())
    }
    fn save_checkpoints(&self, request: &PromptRequest, id: &SessionId, all: &[Checkpoint]) -> Result<(), String> {
        self.checkpoints.lock().map_err(|_| "checkpoints unavailable")?.insert(scoped_id(request, id), all.to_vec());
        Ok(())
    }
    fn load_worktree(
        &self,
        request: &PromptRequest,
        id: &SessionId,
    ) -> Result<Option<crate::agent_runtime::WorktreeState>, String> {
        Ok(self.worktrees.lock().map_err(|_| "worktree store unavailable")?.get(&scoped_id(request, id)).cloned())
    }
    fn save_worktree(
        &self,
        request: &PromptRequest,
        id: &SessionId,
        state: &crate::agent_runtime::WorktreeState,
    ) -> Result<(), String> {
        self.worktrees.lock().map_err(|_| "worktree store unavailable")?.insert(scoped_id(request, id), state.clone());
        Ok(())
    }
    fn forget_worktree(&self, request: &PromptRequest, id: &SessionId) -> Result<(), String> {
        self.worktrees.lock().map_err(|_| "worktree store unavailable")?.remove(&scoped_id(request, id));
        Ok(())
    }
    fn load(&self, id: &SessionId) -> Option<Vec<Message>> {
        self.kept.lock().ok()?.get(id).cloned()
    }

    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String> {
        let mut kept = self.kept.lock().map_err(|_| "the sessions are unavailable")?;
        kept.insert(id.clone(), history.to_vec());
        Ok(())
    }
}

mod file;
mod hold;
mod log;
mod records;
mod repair;

pub use file::FileSessions;
pub use hold::{ANSWERING_ELSEWHERE, Hold};
pub use log::Entry;
pub use records::records;
pub use repair::{INTERRUPTED, repaired};

#[cfg(test)]
mod scope_tests {
    use super::*;

    fn request(cwd: std::path::PathBuf) -> PromptRequest {
        PromptRequest::plain(cwd, "", domain::ModelOption::new("m", "M"))
    }

    #[test]
    fn a_session_keeps_the_id_it_was_saved_under() {
        let dir = std::env::temp_dir();
        let id = SessionId("s".into());
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        let saved = serde_json::to_vec(&("LocalOwner", &canonical, &id)).unwrap_or_default();
        let expected = SessionId(format!("{:x}", Sha256::digest(saved)));
        assert_eq!(scoped_id(&request(dir), &id), expected);
    }

    #[test]
    fn a_session_saved_before_the_owner_was_removed_keeps_its_file_name() {
        const KEPT_AS: &str = "4ba81e1fb4259d80e1a25cbef34478b60acc9b4ba1bb667f76c1fea65b3d584b";
        let cwd = std::path::PathBuf::from("/anchor-never-there/project");
        let id = SessionId("anchor-session".into());
        assert_eq!(scoped_id(&request(cwd.clone()), &id).0, KEPT_AS);
        let dir = tempfile::tempdir().expect("dir");
        FileSessions::new(dir.path()).save_for(&request(cwd), &id, &[]).expect("save");
        assert!(dir.path().join(format!("{KEPT_AS}.jsonl")).is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_whose_name_is_not_utf8_still_has_a_session() {
        use std::os::unix::ffi::OsStrExt;
        let odd = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/odd-\xff"));
        let id = SessionId("s".into());
        assert_eq!(scoped_id(&request(odd.clone()), &id), scoped_id(&request(odd), &id));
    }
}
