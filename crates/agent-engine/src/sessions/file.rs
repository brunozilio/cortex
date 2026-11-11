use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use domain::{EngineEvent, PromptRequest, SessionId};

use super::log::{self, Entry};
use super::{Sessions, scoped_id};
use crate::checkpoints::Checkpoint;
use crate::transcript::Message;

#[derive(Debug)]
pub struct FileSessions {
    dir: PathBuf,
}

impl FileSessions {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn entries(&self, request: &PromptRequest, id: &SessionId) -> Result<Vec<Entry>, String> {
        let id = scoped_id(request, id);
        match read(&self.path(&id, "jsonl")?)? {
            Some(text) => log::all(&text),
            None => Ok(self.legacy(&id)?.unwrap_or_default().into_iter().map(Entry::Message).collect()),
        }
    }

    fn path(&self, id: &SessionId, extension: &str) -> Result<PathBuf, String> {
        let safe = !id.0.is_empty() && id.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        safe.then(|| self.dir.join(format!("{}.{extension}", id.0))).ok_or_else(|| "invalid session id".into())
    }

    fn legacy(&self, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        read(&self.path(id, "json")?)?
            .map(|text| serde_json::from_str(&text).map_err(|e| format!("the saved conversation is invalid: {e}")))
            .transpose()
    }

    fn conversation(&self, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        match read(&self.path(id, "jsonl")?)? {
            Some(text) => log::resume(&text).map(Some),
            None => self.legacy(id),
        }
    }

    fn append(&self, id: &SessionId, entries: &[Entry]) -> Result<(), String> {
        let mut lines = String::new();
        for entry in entries {
            lines.push_str(&serde_json::to_string(entry).map_err(|e| e.to_string())?);
            lines.push('\n');
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.path(id, "jsonl")?;
        let mut file = platform::private_file::open(OpenOptions::new().create(true).append(true), &path)
            .map_err(|e| e.to_string())?;
        file.write_all(lines.as_bytes()).map_err(|e| e.to_string())
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, name: String) -> Result<Option<T>, String> {
        read(&self.dir.join(name))?.map(|text| serde_json::from_str(&text).map_err(|e| e.to_string())).transpose()
    }

    fn write_json(&self, name: String, value: &impl serde::Serialize) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.dir.join(name);
        let partial = path.with_extension(format!("{}.partial", uuid::Uuid::new_v4()));
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        let result = (|| {
            platform::private_file::write(&partial, &bytes)?;
            std::fs::rename(&partial, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(partial);
        }
        result.map_err(|e: std::io::Error| e.to_string())
    }
}

fn read(path: &PathBuf) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("the saved conversation could not be read: {e}")),
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
            Err(error) if error.kind() != ErrorKind::NotFound => Err(error.to_string()),
            _ => Ok(()),
        }
    }
    fn load_checkpoints(&self, request: &PromptRequest, id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        Ok(self.read_json(format!("checkpoints-{}.json", scoped_id(request, id).0))?.unwrap_or_default())
    }
    fn save_checkpoints(&self, request: &PromptRequest, id: &SessionId, all: &[Checkpoint]) -> Result<(), String> {
        self.write_json(format!("checkpoints-{}.json", scoped_id(request, id).0), &all)
    }
    fn load_snapshot(&self, request: &PromptRequest, id: &SessionId) -> Option<crate::prompt::Snapshot> {
        self.read_json(format!("snapshot-{}.json", scoped_id(request, id).0)).ok().flatten()
    }
    fn save_snapshot(
        &self,
        request: &PromptRequest,
        id: &SessionId,
        snapshot: &crate::prompt::Snapshot,
    ) -> Result<(), String> {
        self.write_json(format!("snapshot-{}.json", scoped_id(request, id).0), snapshot)
    }
    fn load_for(&self, request: &PromptRequest, id: &SessionId) -> Option<Vec<Message>> {
        self.load_for_checked(request, id).ok().flatten()
    }
    fn load_for_checked(&self, request: &PromptRequest, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        self.conversation(&scoped_id(request, id))
    }
    fn load(&self, id: &SessionId) -> Option<Vec<Message>> {
        self.conversation(id).ok().flatten()
    }
    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String> {
        let started = read(&self.path(id, "jsonl")?)?.is_some();
        let legacy = if started { None } else { self.legacy(id)? };
        let kept = match &legacy {
            Some(old) => old.clone(),
            None => self.conversation(id)?.unwrap_or_default(),
        };
        let mut entries: Vec<Entry> = legacy.unwrap_or_default().into_iter().map(Entry::Message).collect();
        entries.extend(log::changes(&kept, history));
        self.append(id, &entries)?;
        if !started {
            match std::fs::remove_file(self.path(id, "json")?) {
                Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.to_string()),
                _ => {}
            }
        }
        Ok(())
    }
    fn record(&self, request: &PromptRequest, id: &SessionId, at_ms: u64, event: &EngineEvent) -> Result<(), String> {
        self.append(&scoped_id(request, id), &[Entry::Event { at_ms, event: event.clone() }])
    }
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
