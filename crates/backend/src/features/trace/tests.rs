use agent_engine::sessions::{Entry, Sessions};
use domain::{EngineEvent, OrgId, ToolCall, UserId};

use super::*;

fn recorded(dir: &Path, owner: &Principal) -> (FileSessions, SessionId) {
    let store = FileSessions::new(dir.join("sessions"));
    let id = SessionId("s1".into());
    let mut request = PromptRequest::plain(dir.into(), "", ModelOption::new("", ""));
    request.credentials.owner = Some(owner.clone());
    for entry in [
        Entry::Prompted { at_ms: 1, prompt: "read it".into() },
        Entry::Event { at_ms: 2, event: EngineEvent::ToolStarted(ToolCall::new("r", "Read", "a.txt")) },
        Entry::Event {
            at_ms: 3,
            event: EngineEvent::ToolResult { id: "r".into(), output: "alpha".into(), failed: false },
        },
    ] {
        store.record(&request, &id, &entry).expect("record");
    }
    (store, id)
}

#[test]
fn a_session_is_traced_for_its_owner_in_its_folder() {
    let dir = tempfile::tempdir().expect("dir");
    let (store, id) = recorded(dir.path(), &Principal::LocalOwner);
    let trace = traced(&store, &Principal::LocalOwner, dir.path(), &id).expect("trace");
    let steps = trace["lane"]["steps"].as_array().expect("steps");
    assert_eq!(steps[0]["nodes"][0]["kind"], "prompt");
    assert_eq!(steps[0]["nodes"][0]["text"], "read it");
    assert_eq!(steps[1]["nodes"][0]["output"], "alpha");
}

#[test]
fn another_principal_or_folder_finds_nothing_to_trace() {
    let dir = tempfile::tempdir().expect("dir");
    let (store, id) = recorded(dir.path(), &Principal::LocalOwner);
    let stranger = Principal::member(UserId::new(), OrgId::new());
    assert!(traced(&store, &stranger, dir.path(), &id).is_err());
    assert!(traced(&store, &Principal::LocalOwner, &dir.path().join("elsewhere"), &id).is_err());
}
