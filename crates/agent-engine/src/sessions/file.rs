use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use domain::SessionId;
use platform::store::Document;
use platform::sync::Locked;

use super::durable::{self, Counted, Digests, Persisted, Tail, Written};
use super::log::{self, Entry, Holds, Line};
use super::trace_file::{Paths, Traces};
use super::{Hold, Sessions, scoped_id, seq};
use crate::checkpoints::{self, Checkpoint};
use crate::transcript::Message;

const CONVERSATION: &str = "jsonl";
const TRACE: &str = "trace.jsonl";
const EARLIER_TRACE: &str = "trace.1.jsonl";
const BEFORE_THE_LOG: &str = "json";
const CORRUPT: &str = "jsonl.corrupt";

#[derive(Debug)]
pub struct FileSessions {
    dir: PathBuf,
    persisted: Mutex<HashMap<String, Persisted>>,
    traces: Mutex<Traces>,
    counted: Counted,
    notices: Mutex<Vec<String>>,
}

impl FileSessions {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            persisted: Mutex::default(),
            traces: Mutex::default(),
            counted: Counted::default(),
            notices: Mutex::default(),
        }
    }

    pub fn rotating_the_trace_at(self, bytes: u64) -> Self {
        self.traces.locked().rotating_at(bytes);
        self
    }

    pub fn written(&self) -> Written {
        self.counted.so_far()
    }

    pub fn entries(&self, cwd: &Path, id: &SessionId) -> Result<Vec<Entry>, String> {
        let id = scoped_id(cwd, id);
        let said = match read(&self.path(&id, CONVERSATION)?)? {
            Some(text) => {
                let (said, skipped) = log::lines(&text, Holds::Conversation);
                self.set_aside(&id, &skipped);
                said
            }
            None => self.before_the_log(&id)?.into_iter().flatten().map(unnumbered).collect(),
        };
        Ok(log::merged(said, self.traced(&id)))
    }

    fn traced(&self, id: &SessionId) -> Vec<Line> {
        self.traces.locked().flush(&id.0);
        let mut traced = Vec::new();
        for generation in [EARLIER_TRACE, TRACE] {
            let read = self.path(id, generation).and_then(|path| read(&path));
            match read.map(|text| log::lines(&text.unwrap_or_default(), Holds::TraceOnly).0) {
                Ok(lines) => traced.extend(lines),
                Err(error) => tracing::warn!(%error, "a trace file could not be read; the trace goes on without it"),
            }
        }
        traced
    }

    fn path(&self, id: &SessionId, extension: &str) -> Result<PathBuf, String> {
        let safe = !id.0.is_empty() && id.0.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        safe.then(|| self.dir.join(format!("{}.{extension}", id.0))).ok_or_else(|| "invalid session id".into())
    }

    fn before_the_log(&self, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        read(&self.path(id, BEFORE_THE_LOG)?)?
            .map(|text| serde_json::from_str(&text).map_err(|e| format!("the saved conversation is invalid: {e}")))
            .transpose()
    }

    fn conversation(&self, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        let Some(text) = read(&self.path(id, CONVERSATION)?)? else {
            self.persisted.locked().remove(&id.0);
            return self.before_the_log(id);
        };
        let resumed = log::resume(&text);
        seq::follows(resumed.last_seq);
        self.set_aside(id, &resumed.skipped);
        let whole = (text.is_empty() || text.ends_with('\n'))
            && !text.contains(char::REPLACEMENT_CHARACTER)
            && resumed.skipped.is_empty();
        let mut persisted = self.persisted.locked();
        if whole {
            persisted.insert(id.0.clone(), Persisted::of(&resumed.history, text.len() as u64));
        } else {
            persisted.remove(&id.0);
        }
        Ok(Some(resumed.history))
    }

    fn keep(&self, id: &SessionId, history: &[Message], restart: bool) -> Result<(), String> {
        let path = self.path(id, CONVERSATION)?;
        let on_disk = durable::length(&path).map_err(|e| e.to_string())?;
        let known = self.persisted.locked().get(&id.0).copied().filter(|kept| Some(kept.bytes) == on_disk);
        let digests = Digests::of(history, known.map_or(history.len(), |kept| kept.messages));
        let entries = match known.filter(|kept| digests.start == Some(kept.digest)) {
            _ if restart => log::restarted(history),
            Some(kept) if kept.messages == history.len() => return Ok(()),
            Some(kept) => history[kept.messages..].iter().cloned().map(Entry::Message).collect(),
            None => {
                self.counted.reread();
                log::changes(&self.conversation(id)?.unwrap_or_default(), history)
            }
        };
        let tail = match self.persisted.locked().get(&id.0) {
            Some(kept) if Some(kept.bytes) == on_disk => Tail::KnownWhole,
            _ => Tail::Unknown,
        };
        let bytes = self.append(id, &entries, tail)?;
        let kept = Persisted { messages: history.len(), digest: digests.whole, bytes };
        self.persisted.locked().insert(id.0.clone(), kept);
        Ok(())
    }

    fn append(&self, id: &SessionId, entries: &[Entry], tail: Tail) -> Result<u64, String> {
        let path = self.path(id, CONVERSATION)?;
        let carried = if tail == Tail::Unknown && !path.exists() { self.before_the_log(id)? } else { None };
        let carried_entries: Vec<Entry> = carried.iter().flatten().cloned().map(Entry::Message).collect();
        let mut lines = String::new();
        for entry in carried_entries.iter().chain(entries) {
            lines.push_str(&log::encoded(entry, seq::next())?);
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let bytes = durable::append(&path, &lines, tail, &self.counted).map_err(|e| e.to_string())?;
        if carried.is_some() {
            std::fs::remove_file(self.path(id, BEFORE_THE_LOG)?).map_err(|e| e.to_string())?;
        }
        Ok(bytes)
    }

    fn trace(&self, id: &SessionId, entry: &Entry) -> Result<(), String> {
        let paths = Paths { current: self.path(id, TRACE)?, previous: self.path(id, EARLIER_TRACE)? };
        let mut traces = self.traces.locked();
        let line = log::encoded(entry, seq::next())?;
        traces.append(&id.0, &paths, &line).map_err(|e| e.to_string())
    }

    fn set_aside(&self, id: &SessionId, skipped: &[String]) {
        if skipped.is_empty() {
            return;
        }
        let Ok(path) = self.path(id, CORRUPT) else { return };
        let kept = Document::at(&path).leaving_no_lock_file().update_text(|text| {
            let fresh: Vec<&String> =
                skipped.iter().filter(|line| !text.lines().any(|kept| kept == line.as_str())).collect();
            for line in &fresh {
                text.push_str(line);
                text.push('\n');
            }
            fresh.len()
        });
        match kept {
            Ok(0) => {}
            Ok(count) => self.notices.locked().push(format!(
                "{count} line(s) of the saved conversation could not be read and were left out; they are kept in {}.",
                path.display()
            )),
            Err(error) => {
                tracing::warn!(%error, "a line of the saved conversation that does not parse was not kept aside")
            }
        }
    }

    fn side_file(&self, name: String) -> Document {
        Document::at(self.dir.join(name)).compact().leaving_no_lock_file()
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, name: String) -> Result<Option<T>, String> {
        self.side_file(name).read().map_err(|error| error.to_string())
    }

    fn write_json(&self, name: String, value: &impl serde::Serialize) -> Result<(), String> {
        self.side_file(name).write(value).map_err(|error| error.to_string())
    }
}

fn unnumbered(message: Message) -> Line {
    Line { seq: None, entry: Entry::Message(message) }
}

fn read(path: &PathBuf) -> Result<Option<String>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("the saved conversation could not be read: {e}")),
    }
}

impl Sessions for FileSessions {
    fn load_worktree(&self, cwd: &Path, id: &SessionId) -> Result<Option<crate::agent_runtime::WorktreeState>, String> {
        self.read_json(format!("worktree-{}.json", scoped_id(cwd, id).0))
    }
    fn save_worktree(
        &self,
        cwd: &Path,
        id: &SessionId,
        state: &crate::agent_runtime::WorktreeState,
    ) -> Result<(), String> {
        self.write_json(format!("worktree-{}.json", scoped_id(cwd, id).0), state)
    }
    fn forget_worktree(&self, cwd: &Path, id: &SessionId) -> Result<(), String> {
        let path = self.dir.join(format!("worktree-{}.json", scoped_id(cwd, id).0));
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != ErrorKind::NotFound => Err(error.to_string()),
            _ => Ok(()),
        }
    }
    fn load_checkpoints(&self, cwd: &Path, id: &SessionId) -> Result<Vec<Checkpoint>, String> {
        Ok(self.read_json(format!("checkpoints-{}.json", scoped_id(cwd, id).0))?.unwrap_or_default())
    }
    fn save_checkpoints(&self, cwd: &Path, id: &SessionId, all: &[Checkpoint]) -> Result<(), String> {
        self.write_json(format!("checkpoints-{}.json", scoped_id(cwd, id).0), &all)
    }
    fn add_checkpoint(&self, cwd: &Path, id: &SessionId, checkpoint: Checkpoint) -> Result<(), String> {
        self.side_file(format!("checkpoints-{}.json", scoped_id(cwd, id).0))
            .update(|all: &mut Vec<Checkpoint>| checkpoints::put(all, checkpoint))
            .map_err(|error| error.to_string())
    }
    fn take_notices(&self) -> Vec<String> {
        std::mem::take(&mut *self.notices.locked())
    }
    fn load_snapshot(&self, cwd: &Path, id: &SessionId) -> Option<crate::prompt::Snapshot> {
        self.read_json(format!("snapshot-{}.json", scoped_id(cwd, id).0)).ok().flatten()
    }
    fn save_snapshot(&self, cwd: &Path, id: &SessionId, snapshot: &crate::prompt::Snapshot) -> Result<(), String> {
        self.write_json(format!("snapshot-{}.json", scoped_id(cwd, id).0), snapshot)
    }
    fn load_for(&self, cwd: &Path, id: &SessionId) -> Option<Vec<Message>> {
        self.load_for_checked(cwd, id).ok().flatten()
    }
    fn load_for_checked(&self, cwd: &Path, id: &SessionId) -> Result<Option<Vec<Message>>, String> {
        self.conversation(&scoped_id(cwd, id))
    }
    fn load(&self, id: &SessionId) -> Option<Vec<Message>> {
        self.conversation(id).ok().flatten()
    }
    fn save(&self, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.keep(id, history, false)
    }
    fn save_compacted(&self, cwd: &Path, id: &SessionId, history: &[Message]) -> Result<(), String> {
        self.keep(&scoped_id(cwd, id), history, true)
    }
    fn record(&self, cwd: &Path, id: &SessionId, entry: &Entry) -> Result<(), String> {
        let id = scoped_id(cwd, id);
        if !entry.is_conversation() {
            return self.trace(&id, entry);
        }
        self.persisted.locked().remove(&id.0);
        self.append(&id, std::slice::from_ref(entry), Tail::Unknown).map(|_| ())
    }
    fn flush_trace(&self, cwd: &Path, id: &SessionId) {
        self.traces.locked().flush(&scoped_id(cwd, id).0);
    }
    fn end_trace(&self, cwd: &Path, id: &SessionId) {
        self.traces.locked().close(&scoped_id(cwd, id).0);
    }
    fn hold(&self, cwd: &Path, id: &SessionId) -> Result<Hold, String> {
        super::hold::take(&self.path(&scoped_id(cwd, id), "lock")?)
    }
    fn length(&self, cwd: &Path, id: &SessionId) -> Result<u64, String> {
        let id = scoped_id(cwd, id);
        self.traces.locked().flush(&id.0);
        let mut bytes = 0;
        for file in [CONVERSATION, TRACE, EARLIER_TRACE] {
            bytes += durable::length(&self.path(&id, file)?).map_err(|e| e.to_string())?.unwrap_or(0);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "split_tests.rs"]
mod split_tests;

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod incremental_tests;
