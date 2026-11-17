use std::path::{Path, PathBuf};

use domain::{EngineEvent, SessionId};

use super::*;
use crate::sessions::{Entry, Sessions};
use crate::transcript::{Block, Message};

struct Kept {
    _dir: tempfile::TempDir,
    native: PathBuf,
    store: FileSessions,
    request: PathBuf,
    id: SessionId,
}

impl Kept {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("dir");
        let native = dir.path().join("native");
        let request = dir.path().to_path_buf();
        Self { store: FileSessions::new(&native), native, request, id: SessionId("s1".into()), _dir: dir }
    }

    fn file(&self, extension: &str) -> PathBuf {
        self.native.join(format!("{}.{extension}", scoped_id(&self.request, &self.id).0))
    }

    fn save(&self, history: &[Message]) {
        self.store.save_for(&self.request, &self.id, history).expect("save");
    }

    fn record(&self, entry: &Entry) {
        self.store.record(&self.request, &self.id, entry).expect("record");
    }

    fn entries(&self) -> Vec<Entry> {
        self.store.entries(&self.request, &self.id).expect("entries")
    }

    fn load(&self) -> Vec<Message> {
        self.store.load_for_checked(&self.request, &self.id).expect("readable").expect("kept")
    }

    fn reopened(&self) -> FileSessions {
        FileSessions::new(&self.native)
    }
}

fn reply(text: &str) -> Message {
    Message::assistant(vec![Block::Text(text.into())])
}

fn steered(at_ms: u64) -> Entry {
    Entry::Event { at_ms, event: EngineEvent::Steered(format!("at {at_ms}")) }
}

fn kinds(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a whole line")["kind"].to_string())
        .collect()
}

fn mixed(path: &Path, entries: &[Entry]) {
    std::fs::create_dir_all(path.parent().expect("folder")).expect("folder");
    let text: String =
        entries.iter().map(|entry| format!("{}\n", serde_json::to_string(entry).expect("json"))).collect();
    std::fs::write(path, text).expect("a file from before the split");
}

#[test]
fn a_new_session_keeps_the_conversation_and_the_trace_in_two_files_read_back_as_one_log() {
    let kept = Kept::new();
    kept.record(&steered(1));
    kept.save(&[Message::user("one")]);
    kept.record(&steered(2));
    kept.save(&[Message::user("one"), reply("two")]);
    kept.record(&steered(3));

    assert_eq!(
        kept.entries(),
        [steered(1), Entry::Message(Message::user("one")), steered(2), Entry::Message(reply("two")), steered(3)]
    );
    assert_eq!(kinds(&kept.file("jsonl")), ["\"message\"", "\"message\""]);
    assert_eq!(kinds(&kept.file("trace.jsonl")), ["\"event\"", "\"event\"", "\"event\""]);
    assert_eq!(kept.reopened().entries(&kept.request, &kept.id).expect("entries"), kept.entries());
}

#[test]
fn a_file_from_before_the_split_reads_as_it_always_did() {
    let kept = Kept::new();
    let written = [steered(1), Entry::Message(Message::user("one")), steered(2), Entry::Message(reply("two"))];
    mixed(&kept.file("jsonl"), &written);

    assert_eq!(kept.entries(), written);
    assert_eq!(kept.load(), [Message::user("one"), reply("two")]);
    assert!(!kept.file("trace.jsonl").exists(), "reading moves nothing");
}

#[test]
fn a_file_from_before_the_split_goes_on_with_the_conversation_in_it_and_the_trace_beside_it() {
    let kept = Kept::new();
    let old = [steered(1), Entry::Message(Message::user("one")), steered(2), Entry::Message(reply("two"))];
    mixed(&kept.file("jsonl"), &old);

    kept.record(&steered(3));
    kept.save(&[Message::user("one"), reply("two"), Message::user("three")]);
    kept.record(&steered(4));

    let added = [steered(3), Entry::Message(Message::user("three")), steered(4)];
    assert_eq!(kept.entries(), [old.to_vec(), added.to_vec()].concat());
    assert_eq!(kept.load(), [Message::user("one"), reply("two"), Message::user("three")]);
    assert_eq!(kinds(&kept.file("trace.jsonl")), ["\"event\"", "\"event\""]);
    assert_eq!(kinds(&kept.file("jsonl")).len(), 5, "only the new message was added to the old file");
}

#[test]
fn a_trace_that_is_missing_or_does_not_parse_never_stops_the_conversation() {
    let kept = Kept::new();
    kept.record(&steered(1));
    kept.save(&[Message::user("one"), reply("two")]);
    kept.store.end_trace(&kept.request, &kept.id);

    std::fs::write(kept.file("trace.jsonl"), b"{\"kind\":\"called\",\"at_ms\":\n\xff\xfe not text\n").expect("corrupt");
    assert_eq!(kept.reopened().load_for_checked(&kept.request, &kept.id), Ok(Some(kept.load())));
    assert_eq!(kept.entries(), [Entry::Message(Message::user("one")), Entry::Message(reply("two"))]);

    std::fs::remove_file(kept.file("trace.jsonl")).expect("removed");
    std::fs::create_dir(kept.file("trace.jsonl")).expect("something that cannot be read as a file");
    assert_eq!(kept.load(), [Message::user("one"), reply("two")]);
    assert_eq!(kept.entries().len(), 2, "the conversation is still listed");
    kept.save(&[Message::user("one"), reply("two"), Message::user("three")]);
    assert_eq!(kept.load().len(), 3, "and it still grows");
}

#[test]
fn the_trace_is_written_through_one_open_file_and_reaches_the_disk_when_flushed() {
    let kept = Kept::new();
    kept.record(&steered(1));
    kept.record(&steered(2));
    let on_disk = || std::fs::read_to_string(kept.file("trace.jsonl")).unwrap_or_default().lines().count();
    assert_eq!(on_disk(), 0, "small entries wait in the buffer");

    kept.store.flush_trace(&kept.request, &kept.id);
    assert_eq!(on_disk(), 2);

    kept.record(&steered(3));
    assert_eq!(kept.entries().len(), 3, "a reader in this program sees what is still buffered");
    kept.record(&steered(4));
    kept.store.end_trace(&kept.request, &kept.id);
    assert_eq!(on_disk(), 4);
    assert_eq!(kept.store.written().syncs, 0, "the trace is never synced");
}

#[test]
fn a_trace_cut_mid_write_goes_on_from_a_new_line() {
    let kept = Kept::new();
    kept.record(&steered(1));
    kept.store.end_trace(&kept.request, &kept.id);
    let whole = std::fs::read_to_string(kept.file("trace.jsonl")).expect("trace");
    std::fs::write(kept.file("trace.jsonl"), format!("{whole}{{\"kind\":\"event\",\"at_")).expect("cut");

    let later = kept.reopened();
    later.record(&kept.request, &kept.id, &steered(2)).expect("record");
    assert_eq!(later.entries(&kept.request, &kept.id).expect("entries"), [steered(1), steered(2)]);
}

#[test]
fn a_trace_that_grew_past_its_limit_is_set_aside_once_and_both_parts_are_read() {
    let kept = Kept::new();
    let store = kept.reopened().rotating_the_trace_at(200);
    let record = |at_ms| store.record(&kept.request, &kept.id, &steered(at_ms)).expect("record");
    (1..=4).for_each(record);
    assert!(kept.file("trace.1.jsonl").exists(), "the full file was set aside");
    assert_eq!(store.entries(&kept.request, &kept.id).expect("entries"), (1..=4).map(steered).collect::<Vec<_>>());

    (5..=12).for_each(record);
    let read = store.entries(&kept.request, &kept.id).expect("entries");
    assert_eq!(read.last(), Some(&steered(12)));
    assert!(read.len() < 12, "only one earlier part is kept: {}", read.len());
    assert_eq!(read, (13 - read.len() as u64..=12).map(steered).collect::<Vec<_>>(), "what is kept is in order");
    let files = std::fs::read_dir(&kept.native).expect("dir").count();
    assert_eq!(files, 2, "the trace and one earlier part, nothing else");
}

#[cfg(unix)]
#[test]
fn the_lock_of_a_session_is_removed_when_it_is_let_go() {
    let kept = Kept::new();
    let held = kept.store.hold(&kept.request, &kept.id).expect("held");
    assert!(kept.file("lock").exists());
    assert!(kept.store.hold(&kept.request, &kept.id).is_err(), "still one window at a time");
    drop(held);
    assert!(!kept.file("lock").exists(), "nothing is left behind");
    assert!(kept.store.hold(&kept.request, &kept.id).is_ok());
}

#[cfg(unix)]
#[test]
fn many_windows_taking_and_letting_go_of_a_session_are_never_inside_together() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let kept = Kept::new();
    let (inside, entered) = (AtomicUsize::new(0), AtomicUsize::new(0));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..200 {
                    if let Ok(_held) = kept.store.hold(&kept.request, &kept.id) {
                        assert_eq!(inside.fetch_add(1, Ordering::SeqCst), 0, "two windows held one session");
                        entered.fetch_add(1, Ordering::SeqCst);
                        std::thread::yield_now();
                        inside.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            });
        }
    });
    assert!(entered.load(Ordering::SeqCst) > 0);
    assert!(!kept.file("lock").exists());
}

#[test]
fn a_checkpoint_is_added_in_one_update_and_old_ones_lose_their_files() {
    use crate::checkpoints::{Checkpoint, KEPT_AT_MOST};
    let kept = Kept::new();
    let taken = |prompt: usize| {
        let mut checkpoint = Checkpoint::open(&format!("prompt {prompt}"), &[]);
        checkpoint.files.insert(PathBuf::from(format!("/project/{prompt}.txt")), Some("b25l".into()));
        checkpoint
    };
    std::thread::scope(|scope| {
        for writer in 0..4 {
            let kept = &kept;
            scope.spawn(move || {
                for prompt in 0..15 {
                    kept.store.add_checkpoint(&kept.request, &kept.id, taken(writer * 100 + prompt)).expect("added");
                }
            });
        }
    });

    let all = kept.store.load_checkpoints(&kept.request, &kept.id).expect("checkpoints");
    assert_eq!(all.len(), 60, "none of the concurrent additions was lost, and none is forgotten");
    let (dropped, live): (Vec<_>, Vec<_>) = all.iter().partition(|checkpoint| checkpoint.files_dropped);
    assert_eq!((dropped.len(), live.len()), (10, KEPT_AT_MOST));
    assert!(all[..10].iter().all(|checkpoint| checkpoint.files_dropped && checkpoint.files.is_empty()));
    assert!(live.iter().all(|checkpoint| checkpoint.files.len() == 1));
}

#[test]
fn a_file_s_state_kept_at_its_first_write_survives_a_crash_and_is_reported_not_overwritten_after_a_restart() {
    use std::sync::Arc;

    use crate::checkpoints::{Checkpoint, FileHistory, rewind};
    let kept = Kept::new();
    let work = tempfile::tempdir().expect("work");
    let (a, b) = (work.path().join("a.txt"), work.path().join("b.txt"));
    std::fs::write(&a, "one").expect("a");
    let earlier = Checkpoint::open("earlier", &[]);
    kept.store.add_checkpoint(&kept.request, &kept.id, earlier).expect("earlier");
    let history = vec![Message::user("earlier"), Message::user("done")];
    let opened = Checkpoint::open("edit a, make b", &history);
    let files = FileHistory::default();
    let store = Arc::new(FileSessions::new(&kept.native));
    let (request, id, journaled) = (kept.request.clone(), kept.id.clone(), opened.clone());
    files.journaled(Arc::new(move |kept: &crate::checkpoints::Kept| {
        let so_far = Checkpoint { files: kept.files.clone(), ..journaled.clone() };
        store.add_checkpoint(&request, &id, so_far).expect("kept");
    }));

    files.before_write(&a);
    std::fs::write(&a, "two").expect("a");
    files.before_write(&b);
    std::fs::write(&b, "made").expect("b");
    drop(files);

    let restarted = FileSessions::new(&kept.native);
    let mut all = restarted.load_checkpoints(&kept.request, &kept.id).expect("checkpoints");
    assert_eq!(all.len(), 2, "the turn cut short has one checkpoint, updated in place");
    let mut said = history.clone();
    said.push(Message::user("edit a, make b"));
    assert!(all[1].files.contains_key(&a) && all[1].files.contains_key(&b), "both first states were kept");
    let rewound = rewind(&mut all, 1, &mut said).expect("rewind");
    assert_eq!(std::fs::read_to_string(&a).expect("a"), "two", "what the files became after the crash is unknown");
    assert!(b.exists());
    let changed = |path| domain::Skipped::new(path, domain::LeftAlone::ChangedSince);
    assert_eq!((rewound.restored, rewound.skipped), (Vec::new(), vec![changed(a), changed(b)]));
    assert_eq!(said, history);
}
