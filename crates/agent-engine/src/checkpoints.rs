use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use domain::{LeftAlone, Skipped};
use platform::sync::Locked;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::transcript::{Message, only_results};

const MAX_FILE: u64 = domain::run::events::KEPT_FILE_AT_MOST;
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub left: BTreeMap<PathBuf, Option<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub not_kept: BTreeMap<PathBuf, LeftAlone>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rewinding {
    pub target: Checkpoint,
    pub restored: Vec<PathBuf>,
    pub skipped: Vec<Skipped>,
    pub conversation: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Kept {
    pub files: BTreeMap<PathBuf, Option<String>>,
    pub not_kept: BTreeMap<PathBuf, LeftAlone>,
}

pub type Journal = Arc<dyn Fn(&Kept) + Send + Sync>;

pub fn state_of(path: &Path) -> Option<String> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Ok(found) if found.is_file() => match std::fs::read(path) {
            Ok(bytes) => Some(format!("{:x}", Sha256::digest(bytes))),
            Err(_) => Some("unreadable".to_owned()),
        },
        Ok(found) if found.file_type().is_symlink() => Some("link".to_owned()),
        _ => Some("other".to_owned()),
    }
}

fn written_through(path: &Path) -> PathBuf {
    match std::fs::symlink_metadata(path) {
        Ok(found) if found.file_type().is_symlink() => {
            path.canonicalize().or_else(|_| platform::atomic::resolved(path)).unwrap_or_else(|_| path.to_path_buf())
        }
        _ => path.to_path_buf(),
    }
}

pub fn left_by_the_agent(files: &BTreeMap<PathBuf, Option<String>>) -> BTreeMap<PathBuf, Option<String>> {
    files.keys().map(|path| (path.clone(), state_of(path))).collect()
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
            left: BTreeMap::new(),
            not_kept: BTreeMap::new(),
        }
    }

    pub fn same_turn(&self, other: &Self) -> bool {
        self.prompt == other.prompt
            && self.history_len == other.history_len
            && self.history_hash == other.history_hash
            && self.joined_after == other.joined_after
    }

    fn bytes(&self) -> usize {
        self.files.values().flatten().map(String::len).sum()
    }

    fn drop_files(&mut self) {
        self.files = BTreeMap::new();
        self.left = BTreeMap::new();
        self.not_kept = BTreeMap::new();
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

pub fn put(checkpoints: &mut Vec<Checkpoint>, checkpoint: Checkpoint) {
    match checkpoints.last_mut().filter(|last| last.same_turn(&checkpoint)) {
        Some(last) => *last = checkpoint,
        None => checkpoints.push(checkpoint),
    }
    cap(checkpoints);
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

#[derive(Default)]
pub struct FileHistory {
    kept: Mutex<Kept>,
    journal: Mutex<Option<Journal>>,
}

impl std::fmt::Debug for FileHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileHistory").field("files", &self.kept.locked().files.len()).finish_non_exhaustive()
    }
}

type Before = Result<Option<String>, LeftAlone>;

fn before_of(path: &Path) -> Before {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE => Err(LeftAlone::TooLarge),
        Ok(_) => std::fs::read(path).map(|bytes| Some(STANDARD.encode(bytes))).map_err(|_| LeftAlone::Unreadable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(LeftAlone::Unreadable),
    }
}

impl FileHistory {
    pub fn journaled(&self, journal: Journal) {
        *self.journal.locked() = Some(journal);
    }

    pub fn before_write(&self, path: &Path) -> Option<LeftAlone> {
        self.before_writes(std::slice::from_ref(&path.to_path_buf()));
        self.kept.locked().not_kept.get(&written_through(path)).cloned()
    }

    pub fn before_writes(&self, paths: &[PathBuf]) {
        let mut found = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for path in paths {
            let path = written_through(path);
            let known = {
                let kept = self.kept.locked();
                kept.files.contains_key(&path) || kept.not_kept.contains_key(&path)
            };
            if known || !seen.insert(path.clone()) {
                continue;
            }
            let before = before_of(&path);
            found.push((path, before));
        }
        if !found.is_empty() {
            self.kept(found);
        }
    }

    pub fn record(&self, path: &Path, before: Option<Vec<u8>>) {
        self.record_all(vec![(path.to_owned(), before)]);
    }

    pub fn record_all(&self, found: Vec<(PathBuf, Option<Vec<u8>>)>) {
        let judged = found.into_iter().map(|(path, before)| {
            let before = match before {
                Some(bytes) if bytes.len() as u64 > MAX_FILE => Err(LeftAlone::TooLarge),
                before => Ok(before.map(|bytes| STANDARD.encode(bytes))),
            };
            (path, before)
        });
        self.kept(judged.collect());
    }

    fn kept(&self, found: Vec<(PathBuf, Before)>) {
        let journal = self.journal.locked().clone();
        let mut kept = self.kept.locked();
        let mut added = false;
        for (path, before) in found {
            if kept.files.contains_key(&path) || kept.not_kept.contains_key(&path) {
                continue;
            }
            match before {
                Ok(before) => {
                    kept.files.insert(path, before);
                }
                Err(why) => {
                    kept.not_kept.insert(path, why);
                }
            }
            added = true;
        }
        if let Some(journal) = journal.filter(|_| added) {
            journal(&kept);
        }
    }

    pub fn take(&self) -> Kept {
        self.journal.locked().take();
        std::mem::take(&mut *self.kept.locked())
    }
}

pub fn rewind(
    checkpoints: &mut Vec<Checkpoint>,
    index: usize,
    history: &mut Vec<Message>,
) -> Result<Rewinding, String> {
    if index >= checkpoints.len() {
        return Err("there is no such checkpoint in this conversation".into());
    }
    if checkpoints[index..].iter().any(|checkpoint| checkpoint.files_dropped) {
        return Err(NO_LONGER_KEPT.into());
    }
    let dropped = checkpoints.split_off(index);
    let (restored, skipped) = restore_all(&dropped);
    let target = dropped.into_iter().next().ok_or("there is no such checkpoint")?;
    let before = target.before(history);
    let conversation = before.is_some();
    if let Some(before) = before {
        *history = before;
    }
    Ok(Rewinding { target, restored, skipped, conversation })
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

fn restore_all(dropped: &[Checkpoint]) -> (Vec<PathBuf>, Vec<Skipped>) {
    let mut not_kept: BTreeMap<&PathBuf, &LeftAlone> = BTreeMap::new();
    let mut oldest_before: BTreeMap<&PathBuf, Option<&str>> = BTreeMap::new();
    for checkpoint in dropped {
        for (path, why) in &checkpoint.not_kept {
            not_kept.entry(path).or_insert(why);
        }
        for (path, before) in &checkpoint.files {
            oldest_before.entry(path).or_insert(before.as_deref());
        }
    }
    let mut newest_left: BTreeMap<&PathBuf, Option<&Option<String>>> = BTreeMap::new();
    for checkpoint in dropped.iter().rev() {
        for path in checkpoint.files.keys() {
            newest_left.entry(path).or_insert_with(|| checkpoint.left.get(path));
        }
    }
    let every: std::collections::BTreeSet<&PathBuf> = not_kept.keys().chain(oldest_before.keys()).copied().collect();
    let (mut restored, mut skipped) = (Vec::new(), Vec::new());
    for path in every {
        if let Some(why) = not_kept.get(path) {
            skipped.push(Skipped::new(path, (*why).clone()));
            continue;
        }
        let before = oldest_before.get(path).copied().flatten();
        let now = state_of(path);
        if state_of_content(before).as_ref() == Some(&now) {
            restored.push(path.clone());
            continue;
        }
        let changed_since = newest_left.get(path).copied().flatten().is_none_or(|left| *left != now);
        if changed_since {
            skipped.push(Skipped::new(path, LeftAlone::ChangedSince));
            continue;
        }
        match restore(path, before) {
            Ok(()) => restored.push(path.clone()),
            Err(error) => skipped.push(Skipped::new(path, LeftAlone::Failed(error))),
        }
    }
    (restored, skipped)
}

fn state_of_content(before: Option<&str>) -> Option<Option<String>> {
    match before {
        None => Some(None),
        Some(content) => STANDARD.decode(content).ok().map(|bytes| Some(format!("{:x}", Sha256::digest(bytes)))),
    }
}

fn restore(path: &Path, before: Option<&str>) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("could not restore {}: {error}", path.display());
    match before {
        Some(content) => {
            let bytes = STANDARD.decode(content).map_err(|e| e.to_string())?;
            platform::atomic::replace_entry(path, &bytes).map_err(failed)
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
        checkpoint.files = files.take().files;
        checkpoint.left = left_by_the_agent(&checkpoint.files);
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

        let Rewinding { target, restored, conversation: cut, .. } =
            rewind(&mut checkpoints, 0, &mut history).expect("rewind");
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

        let cut = rewind(&mut checkpoints, 0, &mut history).expect("rewind").conversation;
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
        let cut = rewind(&mut vec![checkpoint], 0, &mut history).expect("rewind").conversation;
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
        let Rewinding { target, conversation: cut, .. } =
            rewind(&mut checkpoints, 1, &mut history).expect("the newest still rewinds");
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

    fn closed(dir: &Path, history: &mut Vec<Message>, prompt: &str, write: &[(&str, &str)]) -> Checkpoint {
        let mut checkpoint = run(dir, history, prompt, write);
        checkpoint.left = left_by_the_agent(&checkpoint.files);
        checkpoint
    }

    #[test]
    fn a_file_the_user_changed_after_the_agent_is_left_alone_and_reported() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one").expect("write");
        std::fs::write(dir.path().join("b.txt"), "one").expect("write");
        let mut history = Vec::new();
        let first = closed(dir.path(), &mut history, "edit", &[("a.txt", "two"), ("b.txt", "two"), ("c.txt", "made")]);
        std::fs::write(dir.path().join("a.txt"), "the user's own").expect("user edit");
        std::fs::write(dir.path().join("c.txt"), "the user's now").expect("user edit");
        let mut checkpoints = vec![first];

        let rewound = rewind(&mut checkpoints, 0, &mut history).expect("rewind");

        assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "the user's own");
        assert_eq!(std::fs::read_to_string(dir.path().join("c.txt")).expect("c"), "the user's now");
        assert_eq!(std::fs::read_to_string(dir.path().join("b.txt")).expect("b"), "one");
        assert_eq!(rewound.restored, [dir.path().join("b.txt")]);
        assert_eq!(
            rewound.skipped,
            [
                Skipped::new(dir.path().join("a.txt"), LeftAlone::ChangedSince),
                Skipped::new(dir.path().join("c.txt"), LeftAlone::ChangedSince)
            ]
        );
    }

    #[test]
    fn a_file_too_large_to_keep_is_said_at_its_write_and_named_with_the_reason_at_the_rewind() {
        let dir = tempfile::tempdir().expect("dir");
        let (big, small) = (dir.path().join("big.bin"), dir.path().join("a.txt"));
        std::fs::write(&big, vec![b'x'; usize::try_from(MAX_FILE).expect("size") + 1]).expect("big");
        std::fs::write(&small, "one").expect("small");
        let files = FileHistory::default();

        assert_eq!(files.before_write(&big), Some(LeftAlone::TooLarge));
        assert_eq!(files.before_write(&small), None);
        assert_eq!(files.before_write(&big), Some(LeftAlone::TooLarge), "said again at every write");
        std::fs::write(&big, "shrunk").expect("write");
        std::fs::write(&small, "two").expect("write");
        files.record_all(vec![(dir.path().join("huge.log"), Some(vec![0; 11 * 1024 * 1024]))]);
        let kept = files.take();
        let mut checkpoint = Checkpoint::open("edit", &[]);
        (checkpoint.files, checkpoint.not_kept) = (kept.files, kept.not_kept);
        checkpoint.left = left_by_the_agent(&checkpoint.files);
        assert_eq!(checkpoint.files.keys().collect::<Vec<_>>(), [&small]);

        let rewound = rewind(&mut vec![checkpoint], 0, &mut Vec::new()).expect("rewind");

        assert_eq!(rewound.restored, std::slice::from_ref(&small));
        assert_eq!(
            rewound.skipped,
            [Skipped::new(&big, LeftAlone::TooLarge), Skipped::new(dir.path().join("huge.log"), LeftAlone::TooLarge)]
        );
        assert_eq!(std::fs::read_to_string(&big).expect("big"), "shrunk", "left as it is");
        assert_eq!(std::fs::read_to_string(&small).expect("small"), "one");
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_could_not_be_read_is_said_at_its_write_and_named_at_the_rewind() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let locked = dir.path().join("locked.txt");
        std::fs::write(&locked, "secret").expect("write");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o200)).expect("mode");
        if std::fs::read(&locked).is_ok() {
            return;
        }
        let files = FileHistory::default();

        assert_eq!(files.before_write(&locked), Some(LeftAlone::Unreadable));
        let kept = files.take();
        let mut checkpoint = Checkpoint::open("edit", &[]);
        (checkpoint.files, checkpoint.not_kept) = (kept.files, kept.not_kept);
        let rewound = rewind(&mut vec![checkpoint], 0, &mut Vec::new()).expect("rewind");

        assert_eq!(rewound.skipped, [Skipped::new(&locked, LeftAlone::Unreadable)]);
        assert!(rewound.restored.is_empty());
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o600)).expect("mode");
    }

    #[test]
    fn a_file_not_kept_by_any_rewound_turn_is_left_alone_even_if_another_turn_kept_it() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one").expect("write");
        let mut history = Vec::new();
        let first = closed(dir.path(), &mut history, "edit", &[("a.txt", "two")]);
        let mut second = closed(dir.path(), &mut history, "grow", &[]);
        second.not_kept.insert(path.clone(), LeftAlone::TooLarge);
        std::fs::write(&path, "grown").expect("write");

        let rewound = rewind(&mut vec![first, second], 0, &mut history).expect("rewind");

        assert_eq!(rewound.skipped, [Skipped::new(&path, LeftAlone::TooLarge)]);
        assert_eq!(std::fs::read_to_string(&path).expect("a"), "grown");
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_put_back_is_reported_and_every_other_file_is_still_restored() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join("sealed")).expect("folder");
        for name in ["sealed/a.txt", "b.txt", "c.txt"] {
            std::fs::write(dir.path().join(name), "one").expect("write");
        }
        let mut history = Vec::new();
        let first =
            closed(dir.path(), &mut history, "edit", &[("b.txt", "two"), ("sealed/a.txt", "two"), ("c.txt", "two")]);
        let sealed = dir.path().join("sealed");
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o555)).expect("mode");
        let mut checkpoints = vec![first];

        let rewound = rewind(&mut checkpoints, 0, &mut history);

        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let rewound = rewound.expect("a failed file does not stop the rewind");
        assert_eq!(rewound.restored, [dir.path().join("b.txt"), dir.path().join("c.txt")]);
        assert_eq!(rewound.skipped.len(), 1, "{:?}", rewound.skipped);
        assert_eq!(rewound.skipped[0].path, sealed.join("a.txt"));
        assert!(
            matches!(&rewound.skipped[0].why, LeftAlone::Failed(error) if error.contains("a.txt")),
            "{:?}",
            rewound.skipped
        );
        assert_eq!(std::fs::read_to_string(dir.path().join("c.txt")).expect("c"), "one");
        assert!(rewound.conversation && history.is_empty() && checkpoints.is_empty());
    }

    #[test]
    fn a_file_already_back_as_it_was_counts_as_restored_so_a_rewind_can_be_tried_again() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one").expect("write");
        let mut history = Vec::new();
        let first = closed(dir.path(), &mut history, "edit", &[("a.txt", "two"), ("b.txt", "new")]);
        std::fs::write(dir.path().join("a.txt"), "one").expect("put back by an earlier attempt");
        std::fs::remove_file(dir.path().join("b.txt")).expect("removed by an earlier attempt");

        let rewound = rewind(&mut vec![first], 0, &mut history).expect("rewind");

        assert_eq!(rewound.restored, [dir.path().join("a.txt"), dir.path().join("b.txt")]);
        assert!(rewound.skipped.is_empty(), "{:?}", rewound.skipped);
    }

    #[cfg(unix)]
    #[test]
    fn a_rewind_replaces_a_link_put_in_a_file_s_place_and_keeps_the_file_s_mode() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, elsewhere) = (tempfile::tempdir().expect("dir"), tempfile::tempdir().expect("elsewhere"));
        let (script, bystander) = (dir.path().join("run.sh"), elsewhere.path().join("profile"));
        std::fs::write(&script, "one").expect("write");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        std::fs::write(&bystander, "kept").expect("bystander");
        let mut history = Vec::new();
        let mut checkpoint = run(dir.path(), &mut history, "edit", &[("run.sh", "two")]);
        let mut checkpoints = vec![checkpoint.clone()];
        rewind(&mut checkpoints, 0, &mut history).expect("rewind");
        assert_eq!(std::fs::metadata(&script).expect("meta").permissions().mode() & 0o777, 0o755);

        std::fs::remove_file(&script).expect("remove");
        std::os::unix::fs::symlink(&bystander, &script).expect("link");
        history.clear();
        checkpoint.left = left_by_the_agent(&checkpoint.files);
        checkpoints = vec![std::mem::take(&mut checkpoint)];
        rewind(&mut checkpoints, 0, &mut history).expect("rewind");

        assert_eq!(std::fs::read_to_string(&bystander).expect("bystander"), "kept", "the link was not followed");
        assert_eq!(std::fs::read_to_string(&script).expect("script"), "one");
    }

    #[test]
    fn what_a_turn_cut_short_wrote_is_kept_at_its_first_write_but_never_rewound_over_the_user() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "one").expect("write");
        type Files = BTreeMap<PathBuf, Option<String>>;
        let stored: Arc<Mutex<Option<Files>>> = Arc::default();
        let files = FileHistory::default();
        let kept = Arc::clone(&stored);
        files.journaled(Arc::new(move |so_far: &Kept| {
            *kept.lock().expect("kept") = Some(so_far.files.clone());
        }));
        let history: Vec<Message> = Vec::new();
        let opened = Checkpoint::open("edit a", &history);

        files.before_write(&path);
        std::fs::write(&path, "two").expect("write");
        drop(files);

        let mut reloaded = opened;
        reloaded.files = stored.lock().expect("kept").clone().expect("kept at the first write");
        assert_eq!(reloaded.files[&path].as_deref(), Some(STANDARD.encode("one").as_str()));
        std::fs::write(&path, "fixed by the user after the crash").expect("user");
        let mut after_restart = vec![Message::user("edit a")];
        let rewound = rewind(&mut vec![reloaded], 0, &mut after_restart).expect("rewind");
        assert_eq!(std::fs::read_to_string(&path).expect("a"), "fixed by the user after the crash");
        assert_eq!(
            (rewound.restored, rewound.skipped),
            (Vec::new(), vec![Skipped::new(path, LeftAlone::ChangedSince)]),
            "unknown after-state: skipped"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_written_through_a_link_is_kept_and_rewound_where_the_link_leads() {
        let (dir, elsewhere) = (tempfile::tempdir().expect("dir"), tempfile::tempdir().expect("elsewhere"));
        let (target, link) = (elsewhere.path().join("real.txt"), dir.path().join("link.txt"));
        std::fs::write(&target, "one").expect("target");
        std::os::unix::fs::symlink(&target, &link).expect("link");
        let files = FileHistory::default();
        files.before_write(&link);
        platform::atomic::write(&link, b"two").expect("write through the link");
        let mut checkpoint = Checkpoint::open("edit", &[]);
        checkpoint.files = files.take().files;
        checkpoint.left = left_by_the_agent(&checkpoint.files);
        let real = target.canonicalize().expect("real");
        assert_eq!(checkpoint.files.keys().collect::<Vec<_>>(), [&real]);

        let rewound = rewind(&mut vec![checkpoint], 0, &mut Vec::new()).expect("rewind");
        assert_eq!(rewound.restored, [real]);
        assert!(std::fs::symlink_metadata(&link).expect("link").file_type().is_symlink(), "the link stays a link");
        assert_eq!(std::fs::read_to_string(&target).expect("target"), "one");
    }

    #[test]
    fn many_files_kept_before_one_command_are_written_down_once() {
        let dir = tempfile::tempdir().expect("dir");
        let paths: Vec<PathBuf> = (0..5_000).map(|n| dir.path().join(format!("f{n}.txt"))).collect();
        for path in &paths {
            std::fs::write(path, "small").expect("write");
        }
        let journal = dir.path().join("journal.json");
        let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let files = FileHistory::default();
        let (counted, written) = (Arc::clone(&writes), journal.clone());
        files.journaled(Arc::new(move |so_far: &Kept| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let bytes = serde_json::to_vec(&so_far.files).expect("json");
            let mut file = std::fs::File::create(&written).expect("journal");
            std::io::Write::write_all(&mut file, &bytes).expect("write");
            file.sync_all().expect("sync");
        }));
        let started = std::time::Instant::now();
        files.before_writes(&paths);
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(files.take().files.len(), 5_000);
    }

    #[test]
    fn taking_the_files_stops_the_journal_so_a_late_write_cannot_empty_the_checkpoint() {
        let dir = tempfile::tempdir().expect("dir");
        let journaled = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let files = FileHistory::default();
        let count = Arc::clone(&journaled);
        files.journaled(Arc::new(move |_: &Kept| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        files.before_write(&dir.path().join("a.txt"));
        files.before_write(&dir.path().join("a.txt"));
        files.record_all(vec![(dir.path().join("b.txt"), None), (dir.path().join("c.txt"), None)]);
        assert_eq!(journaled.load(std::sync::atomic::Ordering::SeqCst), 2, "once per first write, once per batch");
        assert_eq!(files.take().files.len(), 3);
        files.before_write(&dir.path().join("d.txt"));
        assert_eq!(journaled.load(std::sync::atomic::Ordering::SeqCst), 2);
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
        assert_eq!(files.take().files[&path].as_deref(), Some(STANDARD.encode("one").as_str()));
    }
}
