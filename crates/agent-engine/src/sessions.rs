//! Conversations the engine can resume: the transcript of each session,
//! kept between prompts (and, on disk, between launches).

#[cfg(any(test, feature = "test-support"))]
use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use domain::{Principal, PromptRequest, SessionId};

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
    /// The session works in its project again (`ExitWorktree`).
    fn forget_worktree(&self, _request: &PromptRequest, _id: &SessionId) -> Result<(), String> {
        Ok(())
    }
    fn load_for(&self, request: &PromptRequest, id: &SessionId) -> Option<Vec<Message>> {
        self.load(&scoped_id(request, id))
    }
    fn load_for_checked(&self, request: &PromptRequest, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        Ok(self.load_for(request, id))
    }
    fn save_for(&self, request: &PromptRequest, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.save(&scoped_id(request, id), history)
    }
    /// The conversation's checkpoints, oldest first (none when the store
    /// keeps none).
    fn load_checkpoints(&self, _request: &PromptRequest, _id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        Ok(Vec::new())
    }
    fn save_checkpoints(&self, _request: &PromptRequest, _id: &SessionId, _all: &[Checkpoint]) -> Result<(), String> {
        Ok(())
    }
}

fn scoped_id(request: &PromptRequest, id: &SessionId) -> SessionId {
    let cwd = request.cwd.canonicalize().unwrap_or_else(|_| request.cwd.clone());
    let owner = request.credentials.owner.as_ref().unwrap_or(&Principal::LocalOwner);
    let bytes = serde_json::to_vec(&(owner, cwd, id)).expect("session scope serialization");
    SessionId(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Default)]
pub struct InMemorySessions {
    kept: Mutex<HashMap<SessionId, Vec<Message>>>,
    worktrees: Mutex<HashMap<SessionId, crate::agent_runtime::WorktreeState>>,
    checkpoints: Mutex<HashMap<SessionId, Vec<Checkpoint>>>,
}

#[cfg(any(test, feature = "test-support"))]
impl Sessions for InMemorySessions {
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

#[derive(Debug)]
pub struct FileSessions {
    dir: PathBuf,
}

impl FileSessions {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path(&self, id: &SessionId) -> Option<PathBuf> {
        let safe = !id.0.is_empty() && id.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        safe.then(|| self.dir.join(format!("{}.json", id.0)))
    }
}

impl FileSessions {
    fn read_json<T: serde::de::DeserializeOwned>(&self, name: String) -> Result<Option<T>, String> {
        match std::fs::read(self.dir.join(name)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Writes only-the-owner-can-read JSON, whole or not at all.
    fn write_json(&self, name: String, value: &impl serde::Serialize) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.dir.join(name);
        let partial = path.with_extension(format!("{}.partial", uuid::Uuid::new_v4()));
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        let result = (|| {
            private_file::write(&partial, &bytes)?;
            std::fs::rename(&partial, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(partial);
        }
        result.map_err(|e: std::io::Error| e.to_string())
    }
}

impl Sessions for FileSessions {
    fn load_worktree(
        &self,
        request: &PromptRequest,
        id: &SessionId,
    ) -> Result<Option<crate::agent_runtime::WorktreeState>, String> {
        self.read_json(format!("worktree-{}.json", scoped_id(request, id).0))
    }
    fn save_worktree(
        &self,
        request: &PromptRequest,
        id: &SessionId,
        state: &crate::agent_runtime::WorktreeState,
    ) -> Result<(), String> {
        self.write_json(format!("worktree-{}.json", scoped_id(request, id).0), state)
    }
    fn forget_worktree(&self, request: &PromptRequest, id: &SessionId) -> Result<(), String> {
        let path = self.dir.join(format!("worktree-{}.json", scoped_id(request, id).0));
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
            _ => Ok(()),
        }
    }
    fn load_checkpoints(&self, request: &PromptRequest, id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        Ok(self.read_json(format!("checkpoints-{}.json", scoped_id(request, id).0))?.unwrap_or_default())
    }
    fn save_checkpoints(&self, request: &PromptRequest, id: &SessionId, all: &[Checkpoint]) -> Result<(), String> {
        self.write_json(format!("checkpoints-{}.json", scoped_id(request, id).0), &all)
    }
    fn load_for(&self, request: &PromptRequest, id: &SessionId) -> Option<Vec<Message>> {
        self.load_for_checked(request, id).ok().flatten()
    }
    fn load_for_checked(&self, request: &PromptRequest, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        let read = |id: &SessionId| -> Result<Option<Vec<Message>>, String> {
            let path = self.path(id).ok_or("invalid session id")?;
            match std::fs::read(path) {
                Ok(bytes) => serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|e| format!("the saved conversation is invalid: {e}")),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(format!("the saved conversation could not be read: {e}")),
            }
        };
        read(&scoped_id(request, id))
    }
    fn load(&self, id: &SessionId) -> Option<Vec<Message>> {
        let text = std::fs::read_to_string(self.path(id)?).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String> {
        let path = self.path(id).ok_or("invalid session id")?;
        std::fs::create_dir_all(&self.dir).map_err(|error| error.to_string())?;
        let json = serde_json::to_string(history).map_err(|error| error.to_string())?;
        let partial = path.with_extension(format!("{}.partial", uuid::Uuid::new_v4()));
        let result = (|| {
            private_file::write(&partial, json.as_bytes())?;
            std::fs::rename(&partial, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result.map_err(|error: std::io::Error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_sessions_on_disk_and_refuses_odd_ids() {
        let dir = tempfile::tempdir().expect("dir");
        let sessions = FileSessions::new(dir.path().join("native"));
        let id = SessionId("3f2a-77".into());
        sessions.save(&id, &[Message::user("hi")]).expect("save");
        assert_eq!(sessions.load(&id), Some(vec![Message::user("hi")]));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(sessions.path(&id).expect("valid test fixture"))
                    .expect("valid test fixture")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(sessions.load(&SessionId("other".into())), None);
        assert!(sessions.save(&SessionId("../x".into()), &[]).is_err());
    }
}
