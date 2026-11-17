use std::path::Path;

use agent_engine::sessions::{Entry, FileSessions, Sessions};
use domain::trace::Record;
use domain::{EngineEvent, SessionId};

use super::*;

fn kept(store: &FileSessions, dir: &Path, id: &SessionId, entries: Vec<Entry>) {
    for entry in entries {
        store.record(dir, id, &entry).expect("record");
    }
}

#[test]
fn a_sessions_history_is_what_was_asked_and_what_happened_in_order() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let id = SessionId("s1".into());
    kept(
        &store,
        dir.path(),
        &id,
        vec![
            Entry::Prompted { at_ms: 1, prompt: "say hi".into() },
            Entry::Opened { at_ms: 1, run: domain::trace::RunInfo::default() },
            Entry::Event { at_ms: 2, event: EngineEvent::TextDelta("Hi.".into()) },
            Entry::Called { at_ms: 3, agent: None, call: domain::trace::ModelCall::default() },
            Entry::Event { at_ms: 4, event: EngineEvent::Completed { duration_ms: 3, cost_usd: None } },
        ],
    );

    assert_eq!(
        history(&store, dir.path(), &id).expect("history"),
        [
            Record::Prompted { at_ms: 1, prompt: "say hi".into() },
            Record::Event { at_ms: 2, event: EngineEvent::TextDelta("Hi.".into()) },
            Record::Event { at_ms: 4, event: EngineEvent::Completed { duration_ms: 3, cost_usd: None } },
        ]
    );
}

#[test]
fn another_folder_has_no_history_of_it() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let id = SessionId("s1".into());
    kept(&store, dir.path(), &id, vec![Entry::Prompted { at_ms: 1, prompt: "mine".into() }]);

    assert_eq!(history(&store, Path::new("/elsewhere"), &id).expect("history"), []);
}

#[test]
fn a_sessions_length_grows_with_what_is_written() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let id = SessionId("s1".into());
    assert_eq!(length(&store, dir.path(), &id), Ok(0));

    kept(&store, dir.path(), &id, vec![Entry::Prompted { at_ms: 1, prompt: "one".into() }]);
    let once = length(&store, dir.path(), &id).expect("length");
    kept(&store, dir.path(), &id, vec![Entry::Prompted { at_ms: 2, prompt: "two".into() }]);

    assert!(once > 0);
    assert!(length(&store, dir.path(), &id).expect("length") > once);
    assert_eq!(length(&store, Path::new("/elsewhere"), &id), Ok(0));
}
