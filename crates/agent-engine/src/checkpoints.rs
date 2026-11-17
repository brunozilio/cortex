use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use platform::sync::Locked;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::transcript::{Message, only_results};

const MAX_FILE: u64 = 10 * 1024 * 1024;
pub const KEPT_AT_MOST: usize = 50;
pub const KEPT_BYTES_AT_MOST: usize = 64 * 1024 * 1024;
pub const NO_LONGER_KEPT: &str = "that checkpoint is too old to rewind to: a conversation keeps the files of its \
                                  newest 50 checkpoints, and at most 64 MiB of them";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub prompt: String,
    pub history_len: usize,
    pub history_hash: String,
    pub files: BTreeMap<PathBuf, Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined_after: Option<usize>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub files_dropped: bool,
}

impl Checkpoint {
    pub fn open(prompt: &str, history: &[Message]) -> Self {
        Self {
            prompt: prompt.to_owned(),
            history_len: history.len(),
            history_hash: hash(history),
            files: BTreeMap::new(),
            joined_after: history.last().filter(|last| only_results(last)).map(|last| last.blocks.len()),
            files_dropped: false,
        }
    }

    fn bytes(&self) -> usize {
        self.files.values().flatten().map(String::len).sum()
    }

    fn drop_files(&mut self) {
        self.files = BTreeMap::new();
        self.files_dropped = true;
    }

    fn before(&self, history: &[Message]) -> Option<Vec<Message>> {
        let mut before = history.get(..self.history_len)?.to_vec();
        if let (Some(blocks), Some(last)) = (self.joined_after, before.last_mut()) {
            last.blocks.truncate(blocks);
        }
        (hash(&before) == self.history_hash).then_some(before)
    }
}

pub fn cap(checkpoints: &mut [Checkpoint]) {
    cap_at(checkpoints, KEPT_AT_MOST, KEPT_BYTES_AT_MOST);
}

fn cap_at(checkpoints: &mut [Checkpoint], kept_at_most: usize, bytes_at_most: usize) {
    let mut kept = 0;
    let mut bytes = 0;
    for checkpoint in checkpoints.iter_mut().rev() {
        if checkpoint.files_dropped {
            continue;
        }
        bytes += checkpoint.bytes();
        let newest = kept == 0;
        if newest || (kept < kept_at_most && bytes <= bytes_at_most) {
            kept += 1;
        } else {
            kept = kept_at_most;
            checkpoint.drop_files();
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
        let mut files = self.0.locked();
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

    pub fn record(&self, path: &Path, before: Option<Vec<u8>>) {
        if before.as_ref().is_some_and(|bytes| bytes.len() as u64 > MAX_FILE) {
            return;
        }
        let mut files = self.0.locked();
        files.entry(path.to_owned()).or_insert_with(|| before.map(|bytes| STANDARD.encode(bytes)));
    }

    pub fn take(&self) -> BTreeMap<PathBuf, Option<String>> {
        std::mem::take(&mut *self.0.locked())
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
    if checkpoints[index..].iter().any(|checkpoint| checkpoint.files_dropped) {
        return Err(NO_LONGER_KEPT.into());
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
    let before = target.before(history);
    let intact = before.is_some();
    if let Some(before) = before {
        *history = before;
    }
    Ok((target, restored, intact))
}

pub fn branch(
    checkpoints: &[Checkpoint],
    index: usize,
    history: &[Message],
) -> Result<(String, Vec<Message>, Vec<Checkpoint>), String> {
    let target = checkpoints.get(index).ok_or("there is no such checkpoint in this conversation")?;
    let before =
        target.before(history).ok_or("the conversation was compacted since then, so it cannot branch there")?;
    Ok((target.prompt.clone(), before, checkpoints[..index].to_vec()))
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
    use crate::transcript::{Block, Role};

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
    fn a_branch_takes_the_conversation_before_a_prompt_and_leaves_the_files_alone() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one").expect("write");
        let mut history = Vec::new();
        let first = run(dir.path(), &mut history, "edit a", &[("a.txt", "two")]);
        let second = run(dir.path(), &mut history, "edit a again", &[("a.txt", "three")]);
        let checkpoints = vec![first, second];

        let (prompt, kept, carried) = branch(&checkpoints, 1, &history).expect("branch");

        assert_eq!(prompt, "edit a again");
        assert_eq!(kept, history[..2]);
        assert_eq!(carried, checkpoints[..1]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).expect("a"),
            "three",
            "the files stay as they are"
        );
        assert!(branch(&checkpoints, 2, &history).is_err());
        assert!(branch(&checkpoints, 1, &[Message::user("summary")]).is_err(), "a compacted history cannot branch");
    }

    #[test]
    fn a_prompt_joined_to_unanswered_results_is_cut_out_of_their_message_again() {
        let result = Block::ToolResult { id: "t".into(), content: "ran".into(), is_error: false };
        let before = vec![Message::user("go"), Message { role: Role::User, blocks: vec![result] }];
        let mut history = before.clone();
        let checkpoint = Checkpoint::open("and then?", &history);
        crate::transcript::prompted(&mut history, Message::user("and then?"));
        history.push(Message::user("done"));
        assert_eq!(history.len(), 3, "the prompt joined the results");

        let (_, kept, _) = branch(std::slice::from_ref(&checkpoint), 0, &history).expect("branch");
        assert_eq!(kept, before);
        let (_, _, cut) = rewind(&mut vec![checkpoint], 0, &mut history).expect("rewind");
        assert!(cut);
        assert_eq!(history, before);
    }

    fn holding(prompt: &str, bytes: usize) -> Checkpoint {
        let mut checkpoint = Checkpoint::open(prompt, &[]);
        checkpoint.files.insert(PathBuf::from("/project/a.txt"), Some("x".repeat(bytes)));
        checkpoint.files.insert(PathBuf::from("/project/made.txt"), None);
        checkpoint
    }

    #[test]
    fn only_the_newest_checkpoints_keep_their_files_and_every_one_keeps_its_place() {
        let mut checkpoints: Vec<Checkpoint> = (0..5).map(|at| holding(&format!("prompt {at}"), 10)).collect();
        cap_at(&mut checkpoints, 3, 1_000);
        let dropped: Vec<bool> = checkpoints.iter().map(|checkpoint| checkpoint.files_dropped).collect();
        assert_eq!(dropped, [true, true, false, false, false]);
        assert_eq!(checkpoints[0].prompt, "prompt 0", "a dropped checkpoint still says what it was");
        assert!(checkpoints[0].files.is_empty() && checkpoints[4].files.len() == 2);

        checkpoints.push(holding("prompt 5", 10));
        cap_at(&mut checkpoints, 3, 1_000);
        assert_eq!(checkpoints.iter().filter(|checkpoint| !checkpoint.files_dropped).count(), 3);
        assert!(checkpoints[2].files_dropped && !checkpoints[3].files_dropped);
    }

    #[test]
    fn the_oldest_files_go_first_when_together_they_are_too_large_and_the_newest_always_stays() {
        let mut checkpoints = vec![holding("a", 40), holding("b", 40), holding("c", 40)];
        cap_at(&mut checkpoints, 50, 100);
        let dropped: Vec<bool> = checkpoints.iter().map(|checkpoint| checkpoint.files_dropped).collect();
        assert_eq!(dropped, [true, false, false]);

        let mut huge = vec![holding("small", 1), holding("huge", 500)];
        cap_at(&mut huge, 50, 100);
        assert!(huge[0].files_dropped && !huge[1].files_dropped, "the last prompt can always be undone");
    }

    #[test]
    fn rewinding_to_a_checkpoint_whose_files_were_dropped_says_so_and_changes_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one").expect("write");
        let mut history = Vec::new();
        let first = run(dir.path(), &mut history, "edit a", &[("a.txt", "two")]);
        let second = run(dir.path(), &mut history, "edit a again", &[("a.txt", "three")]);
        let mut checkpoints = vec![first, second];
        cap_at(&mut checkpoints, 1, 1_000);
        let said = history.clone();

        assert_eq!(rewind(&mut checkpoints, 0, &mut history).err(), Some(NO_LONGER_KEPT.to_owned()));
        assert_eq!((checkpoints.len(), &history), (2, &said));
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "three");

        assert!(branch(&checkpoints, 0, &history).is_ok(), "a branch needs the conversation, not the files");
        let (target, _, cut) = rewind(&mut checkpoints, 1, &mut history).expect("the newest still rewinds");
        assert_eq!((target.prompt.as_str(), cut), ("edit a again", true));
        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "two");
    }

    #[test]
    fn a_checkpoint_saved_before_the_cap_reads_as_one_that_keeps_its_files() {
        let saved = r#"{"prompt":"p","history_len":0,"history_hash":"h","files":{}}"#;
        let read: Checkpoint = serde_json::from_str(saved).expect("old checkpoint");
        assert!(!read.files_dropped);
        assert!(!serde_json::to_string(&read).expect("json").contains("files_dropped"));
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
