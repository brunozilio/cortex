use super::*;
use crate::{AgentAsk, AskId, AskKind, EngineEvent, FailureKind};

fn permission(id: &str) -> AgentAsk {
    AgentAsk {
        id: AskId(id.into()),
        kind: AskKind::Permission { tool: "Bash".into(), subject: "make".into(), rememberable: true, rule: None },
    }
}

#[test]
fn a_session_says_what_it_is_doing_until_the_run_ends() {
    let mut transcript = Transcript::default();
    assert_eq!(transcript.status(), SessionStatus::Idle);
    transcript.begin();
    assert_eq!(transcript.status(), SessionStatus::Running);
    transcript.apply(&EngineEvent::ThinkingDelta("hm".into()));
    assert_eq!(transcript.status(), SessionStatus::Thinking);
    transcript.apply(&EngineEvent::TextDelta("Done.".into()));
    assert_eq!(transcript.status(), SessionStatus::Running);
    transcript.apply(&EngineEvent::Completed { duration_ms: 10, cost_usd: None });
    assert_eq!(transcript.status(), SessionStatus::Done);
}

#[test]
fn a_run_that_fails_or_is_stopped_ends_that_way() {
    let mut failed = Transcript::default();
    failed.begin();
    failed.apply(&EngineEvent::Failure(FailureKind::UsageLimit { resets_at: Some(1_761_900_000) }));
    failed.apply(&EngineEvent::Failed("limit".into()));
    assert_eq!(failed.status(), SessionStatus::Failed);
    assert_eq!(failed.failure(), Some(FailureKind::UsageLimit { resets_at: Some(1_761_900_000) }));

    let mut stopped = Transcript::default();
    stopped.begin();
    stopped.apply(&EngineEvent::Cancelled);
    assert_eq!(stopped.status(), SessionStatus::Cancelled);

    failed.begin();
    assert_eq!(failed.failure(), None, "a new run forgets the last failure");
}

#[test]
fn a_session_needs_you_while_an_ask_waits() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::Asked(permission("a1")));
    assert_eq!(transcript.status(), SessionStatus::NeedsYou);
    assert_eq!(transcript.ask().map(|pending| pending.ask.id.clone()), Some(AskId("a1".into())));
    assert_eq!(transcript.ask().and_then(|pending| pending.agent), None);

    transcript.apply(&EngineEvent::AskWithdrawn(AskId("other".into())));
    assert_eq!(transcript.status(), SessionStatus::NeedsYou, "another ask's withdrawal leaves this one");

    transcript.apply(&EngineEvent::AskWithdrawn(AskId("a1".into())));
    assert_eq!(transcript.status(), SessionStatus::Running);
    assert!(transcript.ask().is_none());
}

#[test]
fn an_answered_ask_no_longer_waits_and_the_end_of_the_run_drops_one_left_open() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::Asked(permission("a1")));
    transcript.apply(&EngineEvent::Answered { ask: AskId("a1".into()), decision: crate::Decision::Allow });
    assert_eq!(transcript.status(), SessionStatus::Running);

    transcript.apply(&EngineEvent::Asked(AgentAsk { id: AskId("p".into()), kind: AskKind::Plan { plan: "x".into() } }));
    assert_eq!(transcript.status(), SessionStatus::NeedsYou, "a plan waits for approval too");
    transcript.apply(&EngineEvent::Cancelled);
    assert!(transcript.ask().is_none());
    assert_eq!(transcript.status(), SessionStatus::Cancelled);
}

#[test]
fn an_ask_from_a_sub_agent_names_the_agent_that_asks() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::AgentSpawned {
        index: 2,
        kind: "explore".into(),
        summary: "look around".into(),
        prompt: String::new(),
    });
    let ask = permission("a7");
    transcript.apply(&EngineEvent::AgentEvent { index: 2, event: Box::new(EngineEvent::Asked(ask.clone())) });
    transcript.apply(&EngineEvent::Asked(ask));
    assert_eq!(transcript.ask().and_then(|pending| pending.agent), Some(2));
}
