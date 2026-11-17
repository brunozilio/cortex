use std::path::Path;

use agent_engine::sessions::{Entry, FileSessions, Sessions};
use domain::trace::Record;
use domain::{EngineEvent, ModelOption, OrgId, Principal, PromptRequest, SessionId, UserId};

use super::*;

fn kept(store: &FileSessions, dir: &Path, owner: &Principal, id: &SessionId, entries: Vec<Entry>) {
    let mut request = PromptRequest::plain(dir.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
    for entry in entries {
        store.record(&request, id, &entry).expect("record");
    }
}

#[test]
fn a_sessions_history_is_what_was_asked_and_what_happened_in_order() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let (owner, id) = (Principal::LocalOwner, SessionId("s1".into()));
    kept(
        &store,
        dir.path(),
        &owner,
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
        history(&store, &owner, dir.path(), &id).expect("history"),
        [
            Record::Prompted { at_ms: 1, prompt: "say hi".into() },
            Record::Event { at_ms: 2, event: EngineEvent::TextDelta("Hi.".into()) },
            Record::Event { at_ms: 4, event: EngineEvent::Completed { duration_ms: 3, cost_usd: None } },
        ]
    );
}

#[test]
fn another_owner_or_folder_has_no_history_of_it() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let (owner, id) = (Principal::LocalOwner, SessionId("s1".into()));
    kept(&store, dir.path(), &owner, &id, vec![Entry::Prompted { at_ms: 1, prompt: "mine".into() }]);

    let other = Principal::member(UserId::new(), OrgId::new());
    assert_eq!(history(&store, &other, dir.path(), &id).expect("history"), []);
    assert_eq!(history(&store, &owner, Path::new("/elsewhere"), &id).expect("history"), []);
}

#[test]
fn a_sessions_length_grows_with_what_is_written_and_is_its_owner_s_alone() {
    let dir = tempfile::tempdir().expect("dir");
    let store = FileSessions::new(dir.path().join("sessions"));
    let (owner, id) = (Principal::LocalOwner, SessionId("s1".into()));
    assert_eq!(length(&store, &owner, dir.path(), &id), Ok(0));

    kept(&store, dir.path(), &owner, &id, vec![Entry::Prompted { at_ms: 1, prompt: "one".into() }]);
    let once = length(&store, &owner, dir.path(), &id).expect("length");
    kept(&store, dir.path(), &owner, &id, vec![Entry::Prompted { at_ms: 2, prompt: "two".into() }]);

    assert!(once > 0);
    assert!(length(&store, &owner, dir.path(), &id).expect("length") > once);
    let other = Principal::member(UserId::new(), OrgId::new());
    assert_eq!(length(&store, &other, dir.path(), &id), Ok(0));
}
