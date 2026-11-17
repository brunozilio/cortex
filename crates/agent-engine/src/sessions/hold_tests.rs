use domain::SessionId;

use super::*;
use crate::sessions::{FileSessions, Sessions};

fn request(dir: &std::path::Path) -> std::path::PathBuf {
    dir.to_path_buf()
}

#[test]
fn one_window_at_a_time_answers_in_a_conversation() {
    let dir = tempfile::tempdir().expect("dir");
    let sessions = FileSessions::new(dir.path().join("native"));
    let (request, id) = (request(dir.path()), SessionId("s1".into()));

    let first = sessions.hold(&request, &id).expect("the first window holds it");
    let second = sessions.hold(&request, &id);

    assert!(first.held());
    assert!(!Hold::none().held(), "a store without files holds nothing");

    assert_eq!(second.err(), Some(ANSWERING_ELSEWHERE.to_owned()));
    drop(first);
    assert!(sessions.hold(&request, &id).is_ok(), "free again once the first is done");
}

#[test]
fn other_conversations_are_not_held() {
    let dir = tempfile::tempdir().expect("dir");
    let sessions = FileSessions::new(dir.path().join("native"));
    let request = request(dir.path());

    let _first = sessions.hold(&request, &SessionId("s1".into())).expect("held");

    assert!(sessions.hold(&request, &SessionId("s2".into())).is_ok());
}

#[test]
fn the_length_grows_as_the_conversation_is_written() {
    let dir = tempfile::tempdir().expect("dir");
    let sessions = FileSessions::new(dir.path().join("native"));
    let (request, id) = (request(dir.path()), SessionId("s1".into()));
    assert_eq!(sessions.length(&request, &id), Ok(0), "nothing written yet");

    sessions.save_for(&request, &id, &[crate::transcript::Message::user("hi")]).expect("saved");
    let after_one = sessions.length(&request, &id).expect("length");
    sessions
        .save_for(&request, &id, &[crate::transcript::Message::user("hi"), crate::transcript::Message::user("more")])
        .expect("saved");

    assert!(after_one > 0);
    assert!(sessions.length(&request, &id).expect("length") > after_one);
}
