use std::path::PathBuf;

use domain::SessionId;

use super::*;
use crate::sessions::{Entry, Sessions};
use crate::transcript::{Block, Message, Role};

fn staged() -> (tempfile::TempDir, PathBuf, PathBuf, SessionId) {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let request = dir.path().to_path_buf();
    (dir, native, request, SessionId("s1".into()))
}

fn turn(history: &mut Vec<Message>, at: usize) {
    let call =
        Block::ToolUse { id: format!("c{at}"), name: "Read".into(), input: serde_json::json!({"file_path": at}) };
    let result = Block::ToolResult { id: format!("c{at}"), content: "x".repeat(2_000), is_error: false };
    history.push(Message::assistant(vec![call]));
    history.push(Message { role: Role::User, blocks: vec![result] });
}

fn conversation_file(native: &std::path::Path, request: &std::path::Path, id: &SessionId) -> PathBuf {
    native.join(format!("{}.jsonl", scoped_id(request, id).0))
}

#[test]
fn every_save_appends_only_what_is_new_without_reading_the_file_again() {
    const TURNS: usize = 200;
    let (_dir, native, request, id) = staged();
    let store = FileSessions::new(&native);
    let mut history = vec![Message::user("go")];
    store.save_for(&request, &id, &history).expect("save");
    let after_the_prompt = store.written();
    let mut largest_append = 0;
    for at in 0..TURNS {
        let before = store.written().bytes;
        turn(&mut history, at);
        store.save_for(&request, &id, &history).expect("save");
        store.save_for(&request, &id, &history).expect("an unchanged conversation");
        largest_append = largest_append.max(store.written().bytes - before);
    }

    let written = store.written();
    assert_eq!(written.appends, 1 + TURNS as u64, "one append per change, none for an unchanged conversation");
    assert_eq!(written.syncs, written.appends, "one sync per append");
    assert_eq!(written.rereads, after_the_prompt.rereads, "the file is never read again once it is known");
    assert!(written.rereads <= 1);
    let on_disk = std::fs::metadata(conversation_file(&native, &request, &id)).expect("file").len();
    assert_eq!(written.bytes, on_disk, "nothing is written twice");
    assert!(largest_append < 3_000, "an append is the size of the turn, not of the conversation: {largest_append}");
    assert_eq!(FileSessions::new(&native).load_for_checked(&request, &id), Ok(Some(history)));
}

#[test]
fn a_conversation_loaded_in_this_program_grows_without_being_read_again() {
    let (_dir, native, request, id) = staged();
    let history = vec![Message::user("one"), Message::user("two")];
    FileSessions::new(&native).save_for(&request, &id, &history).expect("an earlier program");

    let store = FileSessions::new(&native);
    let mut resumed = store.load_for_checked(&request, &id).expect("readable").expect("kept");
    resumed.push(Message::user("three"));
    store.save_for(&request, &id, &resumed).expect("save");

    assert_eq!(store.written().rereads, 0);
    assert_eq!(store.load_for(&request, &id), Some(resumed));
}

#[test]
fn what_another_program_appended_is_read_before_anything_is_added() {
    let (_dir, native, request, id) = staged();
    let (here, elsewhere) = (FileSessions::new(&native), FileSessions::new(&native));
    let one = vec![Message::user("one")];
    here.save_for(&request, &id, &one).expect("save");

    let two = vec![Message::user("one"), Message::user("two")];
    elsewhere.save_for(&request, &id, &two).expect("save from another program");
    let three = vec![Message::user("one"), Message::user("two"), Message::user("three")];
    here.save_for(&request, &id, &three).expect("save");

    assert_eq!(here.load_for(&request, &id), Some(three.clone()));
    let messages = here.entries(&request, &id).expect("entries");
    assert_eq!(messages.len(), 3, "the message the other program wrote is not written twice: {messages:?}");
    assert!(here.written().rereads >= 2, "a file that changed underneath is read again");

    std::fs::remove_file(conversation_file(&native, &request, &id)).expect("removed");
    here.save_for(&request, &id, &three).expect("save after the file went away");
    assert_eq!(FileSessions::new(&native).load_for(&request, &id), Some(three));
}

#[test]
fn a_reminder_added_to_the_last_kept_message_rewrites_that_message_only() {
    let (_dir, native, request, id) = staged();
    let store = FileSessions::new(&native);
    let mut history = vec![Message::user("go")];
    (0..3).for_each(|at| turn(&mut history, at));
    store.save_for(&request, &id, &history).expect("save");

    if let Some(last) = history.last_mut() {
        last.blocks.push(Block::Reminder("mode: plan".into()));
    }
    history.push(Message::assistant(vec![Block::Text("planned".into())]));
    store.save_for(&request, &id, &history).expect("save");

    let entries = store.entries(&request, &id).expect("entries");
    assert_eq!(entries.len(), 7 + 3, "the seven kept, then the amendment and the two that follow it");
    assert_eq!(entries[7], Entry::Amended { keep: 6 });
    assert_eq!(FileSessions::new(&native).load_for(&request, &id), Some(history));
}

#[test]
fn a_rewind_and_a_compaction_are_kept_as_they_always_were() {
    let (_dir, native, request, id) = staged();
    let store = FileSessions::new(&native);
    let history = vec![Message::user("one"), Message::user("two"), Message::user("three")];
    store.save_for(&request, &id, &history).expect("save");
    store.save_for(&request, &id, &history[..1]).expect("rewound");
    let compacted = vec![Message::user("summary"), Message::user("three")];
    store.save_compacted(&request, &id, &compacted).expect("compacted");
    let grown = [compacted.clone(), vec![Message::user("four")]].concat();
    store.save_for(&request, &id, &grown).expect("save");

    let kinds: Vec<&str> = store
        .entries(&request, &id)
        .expect("entries")
        .iter()
        .map(|entry| match entry {
            Entry::Message(_) => "message",
            Entry::Truncated { .. } => "truncated",
            Entry::Restarted => "restarted",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["message", "message", "message", "truncated", "restarted", "message", "message", "message"]);
    assert_eq!(FileSessions::new(&native).load_for(&request, &id), Some(grown));
}
