use super::*;
use crate::{AgentAsk, AskId, AskKind, EngineEvent, PermissionMode, ToolCall};

fn presented(transcript: &mut Transcript) {
    transcript.begin();
    transcript.apply(&EngineEvent::ToolStarted(ToolCall::new("p1", "ExitPlanMode", "")));
    transcript.apply(&EngineEvent::Asked(AgentAsk {
        id: AskId("p".into()),
        kind: AskKind::Plan { plan: "1. Back up\n2. Migrate".into() },
    }));
}

fn outcome(transcript: &Transcript) -> Option<Outcome> {
    transcript.entries.iter().find_map(|entry| match entry {
        Entry::Plan { outcome, .. } => Some(*outcome),
        _ => None,
    })
}

#[test]
fn a_presented_plan_waits_and_is_not_shown_as_a_tool_call() {
    let mut transcript = Transcript::default();
    presented(&mut transcript);
    assert_eq!(transcript.entries, [Entry::Plan { plan: "1. Back up\n2. Migrate".into(), outcome: Outcome::Waiting }]);
    assert_eq!(transcript.activity.as_deref(), Some("Waiting for the plan's approval"));
}

#[test]
fn an_approved_plan_remembers_the_mode_it_goes_on_in() {
    let mut transcript = Transcript::default();
    presented(&mut transcript);
    transcript.apply(&EngineEvent::ModeChanged(PermissionMode::AcceptEdits));
    assert_eq!(outcome(&transcript), Some(Outcome::Approved(PermissionMode::AcceptEdits)));
    transcript.apply(&EngineEvent::Completed { duration_ms: 5, cost_usd: None });
    assert_eq!(outcome(&transcript), Some(Outcome::Approved(PermissionMode::AcceptEdits)), "the end keeps it");
}

#[test]
fn a_plan_that_was_refused_or_left_open_kept_planning() {
    let mut refused = Transcript::default();
    presented(&mut refused);
    refused.apply(&EngineEvent::ToolResult { id: "p1".into(), output: "not yet".into(), failed: true });
    assert_eq!(outcome(&refused), Some(Outcome::KeptPlanning));

    let mut stopped = Transcript::default();
    presented(&mut stopped);
    stopped.apply(&EngineEvent::Cancelled);
    assert_eq!(outcome(&stopped), Some(Outcome::KeptPlanning));
}

#[test]
fn entering_plan_mode_is_said_once_without_a_tool_line() {
    let mut transcript = Transcript::default();
    transcript.begin();
    transcript.apply(&EngineEvent::ToolStarted(ToolCall::new("e1", "EnterPlanMode", "")));
    transcript.apply(&EngineEvent::ModeChanged(PermissionMode::Plan));
    assert_eq!(transcript.entries, [Entry::Notice("Entered plan mode".into())]);
}
