use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::transcript::Message;

const MAX_FILE: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub prompt: String,
    pub history_len: usize,
    pub history_hash: String,
    pub files: BTreeMap<PathBuf, Option<String>>,
}

impl Checkpoint {
    pub fn open(prompt: &str, history: &[Message]) -> Self {
        Self {
            prompt: prompt.to_owned(),
            history_len: history.len(),
            history_hash: hash(history),
            files: BTreeMap::new(),
        }
    }
}

fn hash(history: &[Message]) -> String {
    let bytes = serde_json::to_vec(history).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Default)]
pub struct FileHistory(Mutex<BTreeMap<PathBuf, Option<String>>>);

impl FileHistory {
    pub fn before_write(&self, path: &Path) {
        let mut files = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if files.contains_key(path) {
            return;
        }
        let before = match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_FILE => return,
            Ok(_) => match std::fs::read(path) {
                Ok(bytes) => Some(STANDARD.encode(bytes)),
                Err(_) => return,
            },
            Err(_) => None,
        };
        files.insert(path.to_owned(), before);
    }

    pub fn take(&self) -> BTreeMap<PathBuf, Option<String>> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

pub fn rewind(
    checkpoints: &mut Vec<Checkpoint>,
    index: usize,
    history: &mut Vec<Message>,
) -> Result<(Checkpoint, Vec<PathBuf>, bool), String> {
    if index >= checkpoints.len() {
        return Err("there is no such checkpoint in this conversation".into());
    }
    let dropped = checkpoints.split_off(index);
    let mut restored = Vec::new();
    for checkpoint in dropped.iter().rev() {
        for (path, before) in &checkpoint.files {
            restore(path, before.as_deref())?;
            if !restored.contains(path) {
                restored.push(path.clone());
            }
        }
    }
    restored.sort();
    let target = dropped.into_iter().next().ok_or("there is no such checkpoint")?;
    let intact = history.len() >= target.history_len && hash(&history[..target.history_len]) == target.history_hash;
    if intact {
        history.truncate(target.history_len);
    }
    Ok((target, restored, intact))
}

fn restore(path: &Path, before: Option<&str>) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("could not restore {}: {error}", path.display());
    match before {
        Some(content) => {
            let bytes = STANDARD.decode(content).map_err(|e| e.to_string())?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(failed)?;
            }
            std::fs::write(path, bytes).map_err(failed)
        }
        None => match std::fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(failed(error)),
            _ => Ok(()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, history: &mut Vec<Message>, prompt: &str, write: &[(&str, &str)]) -> Checkpoint {
        let mut checkpoint = Checkpoint::open(prompt, history);
        let files = FileHistory::default();
        for (name, content) in write {
            let path = dir.join(name);
            files.before_write(&path);
            std::fs::write(&path, content).expect("write");
        }
        checkpoint.files = files.take();
        history.push(Message::user(prompt));
        history.push(Message::user("done"));
        checkpoint
    }

    #[test]
    fn a_rewind_restores_files_and_cuts_the_conversation_before_its_prompt() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one").expect("write");
        let mut history = Vec::new();
        let first = run(dir.path(), &mut history, "edit a", &[("a.txt", "two")]);
        let second = run(dir.path(), &mut history, "edit a again, add b", &[("a.txt", "three"), ("b.txt", "new")]);
        let mut checkpoints = vec![first, second];

        let (target, restored, cut) = rewind(&mut checkpoints, 0, &mut history).expect("rewind");
        assert_eq!(target.prompt, "edit a");
        assert!(cut);
        assert!(history.is_empty());
        assert!(checkpoints.is_empty());
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "one");
        assert!(!dir.path().join("b.txt").exists(), "a file the agent made is removed");
        assert_eq!(restored, [dir.path().join("a.txt"), dir.path().join("b.txt")]);
    }

    #[test]
    fn a_compacted_conversation_keeps_its_history_and_still_gets_its_files_back() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one").expect("write");
        let mut history = vec![Message::user("earlier")];
        let mut checkpoints = vec![run(dir.path(), &mut history, "edit a", &[("a.txt", "two")])];
        history = vec![Message::user("summary of everything")];

        let (_, _, cut) = rewind(&mut checkpoints, 0, &mut history).expect("rewind");
        assert!(!cut);
        assert_eq!(history, [Message::user("summary of everything")]);
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "one");
        assert!(rewind(&mut checkpoints, 0, &mut history).is_err(), "nothing left to rewind to");
    }

    #[test]
    fn only_a_file_s_first_state_is_kept() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one").expect("write");
        let files = FileHistory::default();
        files.before_write(&path);
        std::fs::write(&path, "two").expect("write");
        files.before_write(&path);
        assert_eq!(files.take()[&path].as_deref(), Some(STANDARD.encode("one").as_str()));
    }
}
