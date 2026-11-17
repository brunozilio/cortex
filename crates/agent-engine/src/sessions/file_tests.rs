use domain::{EngineEvent, SessionId};

use super::*;
use crate::sessions::{Entry, Sessions};
use crate::transcript::{Block, Message};

fn request(dir: &std::path::Path) -> PathBuf {
    dir.to_path_buf()
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
    let steered = Entry::Event { at_ms: 1_700_000_000_000, event: EngineEvent::Steered("faster".into()) };
    store.record(&request, &id, &steered).expect("record");

    assert_eq!(store.load_for(&request, &id), Some(vec![Message::user("one")]));
    assert_eq!(store.entries(&request, &id).expect("entries").last(), Some(&steered));
}

#[test]
fn recording_into_a_conversation_saved_in_the_old_format_keeps_the_conversation() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    std::fs::create_dir_all(&native).expect("dir");
    let legacy = native.join(format!("{}.json", scoped_id(&request, &id).0));
    std::fs::write(&legacy, serde_json::to_vec(&[Message::user("old")]).expect("json")).expect("old file");

    let prompted = Entry::Prompted { at_ms: 1, prompt: "new".into() };
    store.record(&request, &id, &prompted).expect("record");
    assert_eq!(store.load_for(&request, &id), Some(vec![Message::user("old")]));
    assert_eq!(store.entries(&request, &id).expect("entries"), [Entry::Message(Message::user("old")), prompted]);
    assert!(legacy.exists(), "the trace has its own file, so the old conversation is left as it was");
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
fn a_corrupt_line_is_kept_aside_once_and_said_and_the_conversation_loads_without_it() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    store.save_for(&request, &id, &[Message::user("one")]).expect("save");
    let saved = std::fs::read_dir(&native).expect("dir").flatten().next().expect("one file").path();
    let whole = std::fs::read_to_string(&saved).expect("read");
    let after = serde_json::to_string(&Entry::Message(reply("two"))).expect("json");
    std::fs::write(&saved, format!("{whole}{{broken\n{after}\n")).expect("corrupt");

    assert_eq!(store.load_for_checked(&request, &id), Ok(Some(vec![Message::user("one"), reply("two")])));
    let notices = store.take_notices();
    assert!(notices.len() == 1 && notices[0].contains("could not be read"), "{notices:?}");
    let aside = saved.with_extension("jsonl.corrupt");
    assert_eq!(std::fs::read_to_string(&aside).expect("kept aside"), "{broken\n");

    assert!(store.load_for_checked(&request, &id).is_ok());
    assert!(store.take_notices().is_empty(), "said once");
    assert_eq!(std::fs::read_to_string(&aside).expect("kept aside"), "{broken\n", "kept once");
    let grown = vec![Message::user("one"), reply("two"), Message::user("three")];
    store.save_for(&request, &id, &grown).expect("save after the skipped line");
    assert_eq!(store.load_for_checked(&request, &id), Ok(Some(grown)));
}

fn cut_mid_write(native: &std::path::Path) -> std::path::PathBuf {
    let saved = std::fs::read_dir(native).expect("dir").flatten().next().expect("one file").path();
    let whole = std::fs::read_to_string(&saved).expect("read");
    let line = serde_json::to_string(&Entry::Message(reply("never finished"))).expect("json");
    std::fs::write(&saved, format!("{whole}{}", &line[..line.len() / 2])).expect("cut");
    saved
}

#[test]
fn a_last_line_cut_mid_write_is_dropped_and_the_conversation_resumes_and_grows() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    let kept = vec![Message::user("one"), reply("two")];
    store.save_for(&request, &id, &kept).expect("save");
    cut_mid_write(&native);

    assert_eq!(store.load_for_checked(&request, &id), Ok(Some(kept.clone())));
    assert_eq!(messages(store.entries(&request, &id).expect("entries")), kept);
    let grown = [kept, vec![Message::user("three")]].concat();
    store.save_for(&request, &id, &grown).expect("save after the cut");
    assert_eq!(store.load_for_checked(&request, &id), Ok(Some(grown)), "the cut line does not swallow the next one");
}

#[test]
fn a_last_line_cut_inside_a_character_is_dropped_and_saving_goes_on() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    let kept = vec![Message::user("one")];
    store.save_for(&request, &id, &kept).expect("save");
    let saved = std::fs::read_dir(&native).expect("dir").flatten().next().expect("one file").path();
    let mut bytes = std::fs::read(&saved).expect("read");
    let line = serde_json::to_string(&Entry::Message(reply("olá, ação"))).expect("json");
    let inside = line.find('ç').expect("a character of two bytes") + 1;
    bytes.extend_from_slice(&line.as_bytes()[..inside]);
    std::fs::write(&saved, &bytes).expect("cut inside a character");

    let reopened = FileSessions::new(&native);
    let grown = [kept, vec![reply("after the cut")]].concat();
    reopened.save_for(&request, &id, &grown).expect("saving goes on after a cut inside a character");
    assert_eq!(reopened.load_for_checked(&request, &id), Ok(Some(grown)));
}

#[test]
fn a_corrupt_line_before_the_last_one_is_skipped_and_the_last_one_kept() {
    let dir = tempfile::tempdir().expect("dir");
    let native = dir.path().join("native");
    let store = FileSessions::new(&native);
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    store.save_for(&request, &id, &[Message::user("one")]).expect("save");
    let saved = cut_mid_write(&native);
    let cut = std::fs::read_to_string(&saved).expect("read");
    let line = serde_json::to_string(&Entry::Message(reply("after"))).expect("json");
    std::fs::write(&saved, format!("{cut}\n{line}\n")).expect("corrupt");
    assert_eq!(store.load_for_checked(&request, &id), Ok(Some(vec![Message::user("one"), reply("after")])));
}
