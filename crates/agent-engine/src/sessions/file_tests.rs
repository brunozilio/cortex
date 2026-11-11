use domain::{EngineEvent, ModelOption, PromptRequest, SessionId};

use super::*;
use crate::sessions::{Entry, Sessions};
use crate::transcript::{Block, Message};

fn request(dir: &std::path::Path) -> PromptRequest {
    PromptRequest::plain(dir.into(), "", ModelOption::new("m", "M"))
}

fn reply(text: &str) -> Message {
    Message::assistant(vec![Block::Text(text.into())])
}

fn messages(entries: Vec<Entry>) -> Vec<Message> {
    entries
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Message(message) => Some(message),
            _ => None,
        })
        .collect()
}

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
        let saved = std::fs::read_dir(dir.path().join("native")).expect("dir").flatten().next().expect("one file");
        assert_eq!(saved.metadata().expect("metadata").permissions().mode() & 0o777, 0o600);
    }
    assert_eq!(sessions.load(&SessionId("other".into())), None);
    assert!(sessions.save(&SessionId("../x".into()), &[]).is_err());
}

#[test]
fn a_compacted_conversation_resumes_from_the_summary_and_keeps_everything_before_it() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("native"));
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    store.save_for(&request, &id, &[Message::user("one"), reply("two")]).expect("save");
    store.save_for(&request, &id, &[Message::user("one"), reply("two"), Message::user("three")]).expect("save");
    store.save_for(&request, &id, &[Message::user("summary")]).expect("compacted");

    assert_eq!(store.load_for(&request, &id), Some(vec![Message::user("summary")]));
    assert_eq!(
        store.entries(&request, &id).expect("entries"),
        vec![
            Entry::Message(Message::user("one")),
            Entry::Message(reply("two")),
            Entry::Message(Message::user("three")),
            Entry::Restarted,
            Entry::Message(Message::user("summary")),
        ]
    );
}

#[test]
fn a_rewound_conversation_resumes_without_the_undone_turns_but_keeps_them_on_file() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("native"));
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    store.save_for(&request, &id, &[Message::user("one"), reply("two"), Message::user("three")]).expect("save");
    store.save_for(&request, &id, &[Message::user("one")]).expect("rewound");
    store.save_for(&request, &id, &[Message::user("one"), Message::user("again")]).expect("save");

    assert_eq!(store.load_for(&request, &id), Some(vec![Message::user("one"), Message::user("again")]));
    let kept = store.entries(&request, &id).expect("entries");
    assert!(kept.contains(&Entry::Truncated { keep: 1 }), "{kept:?}");
    assert_eq!(messages(kept).len(), 4, "the undone turns stay on file");
}

#[test]
fn events_are_kept_beside_the_conversation_without_entering_it() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("native"));
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    store.save_for(&request, &id, &[Message::user("one")]).expect("save");
    store.record(&request, &id, 1_700_000_000_000, &EngineEvent::Steered("faster".into())).expect("record");

    assert_eq!(store.load_for(&request, &id), Some(vec![Message::user("one")]));
    assert_eq!(
        store.entries(&request, &id).expect("entries").last(),
        Some(&Entry::Event { at_ms: 1_700_000_000_000, event: EngineEvent::Steered("faster".into()) })
    );
}

#[test]
fn a_conversation_saved_in_the_old_format_still_resumes_and_is_kept_when_it_grows() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    let old = vec![Message::user("one"), reply("two")];
    std::fs::create_dir_all(&native).expect("dir");
    let legacy = native.join(format!("{}.json", scoped_id(&request, &id).0));
    std::fs::write(&legacy, serde_json::to_vec(&old).expect("json")).expect("old file");

    assert_eq!(store.load_for(&request, &id), Some(old.clone()));
    let grown = [old.clone(), vec![Message::user("three")]].concat();
    store.save_for(&request, &id, &grown).expect("save");
    assert_eq!(store.load_for(&request, &id), Some(grown.clone()));
    assert_eq!(messages(store.entries(&request, &id).expect("entries")), grown);
    assert!(!legacy.exists(), "the old file is folded into the new one");
}

#[test]
fn a_corrupt_line_fails_the_load_instead_of_losing_the_conversation() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    store.save_for(&request, &id, &[Message::user("one")]).expect("save");
    let saved = std::fs::read_dir(&native).expect("dir").flatten().next().expect("one file").path();
    std::fs::write(&saved, "{broken\n").expect("corrupt");
    assert!(store.load_for_checked(&request, &id).is_err());
}
