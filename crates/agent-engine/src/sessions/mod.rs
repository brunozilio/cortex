#[cfg(any(test, feature = "test-support"))]
use std::collections::HashMap;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;

use domain::legacy::LEGACY_OWNER_TAG;
use std::path::Path;

use domain::SessionId;
use sha2::{Digest, Sha256};

use crate::checkpoints::Checkpoint;
use crate::transcript::Message;

pub trait Sessions: Send + Sync {
    fn load(&self, id: &SessionId) -> Option<Vec<Message>>;
    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String>;
    fn load_worktree(
        &self,
        _cwd: &Path,
        _id: &SessionId,
    ) -> Result<Option<crate::agent_runtime::WorktreeState>, String> {
        Ok(None)
    }
    fn save_worktree(
        &self,
        _cwd: &Path,
        _id: &SessionId,
        _state: &crate::agent_runtime::WorktreeState,
    ) -> Result<(), String> {
        Err("session store does not support persistent worktrees".into())
    }
    fn forget_worktree(&self, _cwd: &Path, _id: &SessionId) -> Result<(), String> {
        Ok(())
    }
    fn load_for(&self, cwd: &Path, id: &SessionId) -> Option<Vec<Message>> {
        self.load(&scoped_id(cwd, id))
    }
    fn record(&self, _cwd: &Path, _id: &SessionId, _entry: &Entry) -> Result<(), String> {
        Ok(())
    }
    fn flush_trace(&self, _cwd: &Path, _id: &SessionId) {}
    fn end_trace(&self, _cwd: &Path, _id: &SessionId) {}
    fn hold(&self, _cwd: &Path, _id: &SessionId) -> Result<Hold, String> {
        Ok(Hold::none())
    }
    fn length(&self, _cwd: &Path, _id: &SessionId) -> Result<u64, String> {
        Ok(0)
    }
    fn load_for_checked(&self, cwd: &Path, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        Ok(self.load_for(cwd, id))
    }
    fn save_for(&self, cwd: &Path, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.save(&scoped_id(cwd, id), history)
    }
    fn save_compacted(&self, cwd: &Path, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.save_for(cwd, id, history)
    }
    fn load_checkpoints(&self, _cwd: &Path, _id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        Ok(Vec::new())
    }
    fn save_checkpoints(&self, _cwd: &Path, _id: &SessionId, _all: &[Checkpoint]) -> Result<(), String> {
        Ok(())
    }
    fn add_checkpoint(&self, cwd: &Path, id: &SessionId, checkpoint: Checkpoint) -> Result<(), String> {
        let mut all = self.load_checkpoints(cwd, id)?;
        crate::checkpoints::put(&mut all, checkpoint);
        self.save_checkpoints(cwd, id, &all)
    }
    fn load_snapshot(&self, _cwd: &Path, _id: &SessionId) -> Option<crate::prompt::Snapshot> {
        None
    }
    fn take_notices(&self) -> Vec<String> {
        Vec::new()
    }
    fn save_snapshot(&self, _cwd: &Path, _id: &SessionId, _snapshot: &crate::prompt::Snapshot) -> Result<(), String> {
        Ok(())
    }
}

fn scoped_id(cwd: &Path, id: &SessionId) -> SessionId {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
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
    fn load_snapshot(&self, cwd: &Path, id: &SessionId) -> Option<crate::prompt::Snapshot> {
        self.snapshots.lock().ok()?.get(&scoped_id(cwd, id)).cloned()
    }
    fn save_snapshot(&self, cwd: &Path, id: &SessionId, snapshot: &crate::prompt::Snapshot) -> Result<(), String> {
        self.snapshots.lock().map_err(|_| "snapshots unavailable")?.insert(scoped_id(cwd, id), snapshot.clone());
        Ok(())
    }
    fn load_checkpoints(&self, cwd: &Path, id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        let kept = self.checkpoints.lock().map_err(|_| "checkpoints unavailable")?;
        Ok(kept.get(&scoped_id(cwd, id)).cloned().unwrap_or_default())
    }
    fn save_checkpoints(&self, cwd: &Path, id: &SessionId, all: &[Checkpoint]) -> Result<(), String> {
        self.checkpoints.lock().map_err(|_| "checkpoints unavailable")?.insert(scoped_id(cwd, id), all.to_vec());
        Ok(())
    }
    fn load_worktree(&self, cwd: &Path, id: &SessionId) -> Result<Option<crate::agent_runtime::WorktreeState>, String> {
        Ok(self.worktrees.lock().map_err(|_| "worktree store unavailable")?.get(&scoped_id(cwd, id)).cloned())
    }
    fn save_worktree(
        &self,
        cwd: &Path,
        id: &SessionId,
        state: &crate::agent_runtime::WorktreeState,
    ) -> Result<(), String> {
        self.worktrees.lock().map_err(|_| "worktree store unavailable")?.insert(scoped_id(cwd, id), state.clone());
        Ok(())
    }
    fn forget_worktree(&self, cwd: &Path, id: &SessionId) -> Result<(), String> {
        self.worktrees.lock().map_err(|_| "worktree store unavailable")?.remove(&scoped_id(cwd, id));
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

mod durable;
mod file;
mod hold;
mod log;
mod records;
mod repair;
mod seq;
mod trace_file;

pub use durable::Written;
pub use file::FileSessions;
pub use hold::{ANSWERING_ELSEWHERE, Hold};
pub use log::Entry;
pub use records::records;
pub use repair::{INTERRUPTED, repaired};

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn a_session_keeps_the_id_it_was_saved_under() {
        let dir = std::env::temp_dir();
        let id = SessionId("s".into());
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        let saved = serde_json::to_vec(&("LocalOwner", &canonical, &id)).unwrap_or_default();
        let expected = SessionId(format!("{:x}", Sha256::digest(saved)));
        assert_eq!(scoped_id(&dir, &id), expected);
    }

    #[test]
    fn a_session_saved_before_the_owner_was_removed_keeps_its_file_name() {
        const KEPT_AS: &str = "4ba81e1fb4259d80e1a25cbef34478b60acc9b4ba1bb667f76c1fea65b3d584b";
        let cwd = std::path::PathBuf::from("/anchor-never-there/project");
        let id = SessionId("anchor-session".into());
        assert_eq!(scoped_id(&cwd, &id).0, KEPT_AS);
        let dir = tempfile::tempdir().expect("dir");
        FileSessions::new(dir.path()).save_for(&cwd, &id, &[]).expect("save");
        assert!(dir.path().join(format!("{KEPT_AS}.jsonl")).is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_whose_name_is_not_utf8_still_has_a_session() {
        use std::os::unix::ffi::OsStrExt;
        let odd = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/odd-\xff"));
        let id = SessionId("s".into());
        assert_eq!(scoped_id(&odd, &id), scoped_id(&odd, &id));
    }
}
