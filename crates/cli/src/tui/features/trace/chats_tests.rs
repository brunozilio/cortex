use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::{AgentAsk, AskId, AskKind, Decision, EngineEvent, Model, SessionId};
use console::{ChatError, Chats, Live};
use serde_json::Value;

use super::PhoneChats;

fn asked() -> EngineEvent {
    EngineEvent::Asked(AgentAsk {
        id: AskId("a1".into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "cargo test".into(), rememberable: true, rule: None },
    })
}

fn until(chats: &PhoneChats, session: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + crate::tui::tests::SETTLES_WITHIN;
    loop {
        let shown = chats.open(session).expect("open");
        if done(&shown) {
            return shown;
        }
        assert!(Instant::now() < deadline, "the chat never got there: {shown}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn said(shown: &Value, kind: &str, text: &str) -> bool {
    shown["entries"].as_array().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry["kind"] == kind && entry["data"].as_str().is_some_and(|data| data.contains(text)))
    })
}

#[test]
fn a_phone_starts_a_chat_answers_its_question_and_reads_the_reply() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s1".into())),
        asked(),
        EngineEvent::Answered { ask: AskId("a1".into()), decision: Decision::Allow },
        EngineEvent::TextDelta("All tests pass.".into()),
        EngineEvent::Completed { duration_ms: 5, cost_usd: None },
    ];
    let engine =
        FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], script).with_delay(Duration::from_millis(50));
    let client = crate::tui::tests::client(&engine, data.path());
    let chats = PhoneChats::new(client, data.path().to_owned(), Live::default());

    let session = chats.start("run the tests").expect("start");
    assert_eq!(session, "s1");
    let waiting = until(&chats, "s1", |shown| shown["ask"]["id"] == "a1");
    assert!(said(&waiting, "user", "run the tests"), "{waiting}");
    assert!(matches!(chats.send("s1", "and lint"), Err(ChatError::Busy(_))), "one prompt at a time");

    chats.answer("s1", "a1", "allow").expect("answer");
    let answered = until(&chats, "s1", |shown| said(shown, "assistant", "All tests pass."));
    assert!(answered["ask"].is_null(), "{answered}");
    assert_eq!(engine.answers(), [(AskId("a1".into()), Decision::Allow)]);
}

#[test]
fn a_phone_cannot_answer_a_chat_that_waits_for_nothing() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![Model::new("keyed:scripted", "Scripted")], Vec::new());
    let chats =
        PhoneChats::new(crate::tui::tests::client(&engine, data.path()), data.path().to_owned(), Live::default());

    assert!(matches!(chats.answer("s1", "a1", "allow"), Err(ChatError::Missing(_))));
    assert!(matches!(chats.answer("s1", "a1", "maybe"), Err(ChatError::Failed(_))));
    assert!(matches!(chats.open("nowhere"), Err(ChatError::Missing(_))));
}
