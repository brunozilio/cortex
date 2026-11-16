use super::*;
use crate::{Decision, EngineEvent};

#[test]
fn every_twentieth_request_is_held_out() {
    let mut card = Scorecard::default();
    let held: Vec<u32> = (1..=60).filter(|_| card.next_held_out()).collect();
    assert_eq!(held.len(), 3);
    assert_eq!(card.briefed, 60);
}

#[test]
fn refusals_with_a_note_and_redirections_count_as_corrections() {
    let mut outcome = RunOutcome::default();
    for event in [
        EngineEvent::Steered("no, use the other file".into()),
        EngineEvent::Answered { ask: crate::AskId("a".into()), decision: Decision::DenyWith("not that".into()) },
        EngineEvent::Answered { ask: crate::AskId("b".into()), decision: Decision::Deny },
        EngineEvent::Answered { ask: crate::AskId("c".into()), decision: Decision::Allow },
        EngineEvent::Completed { duration_ms: 1, cost_usd: Some(0.25) },
    ] {
        outcome.observe(&event);
    }
    assert_eq!(outcome, RunOutcome { completed: true, corrections: 3, cost_usd: Some(0.25) });
}

fn outcome(corrections: u32, cost: f64) -> RunOutcome {
    RunOutcome { completed: true, corrections, cost_usd: Some(cost) }
}

#[test]
fn the_report_compares_runs_with_and_without_memory() {
    let mut card = Scorecard::default();
    for _ in 0..10 {
        card.record(false, &outcome(0, 0.10));
    }
    for _ in 0..5 {
        card.record(true, &outcome(1, 0.20));
    }
    let report = card.report();
    assert!(report.contains("With memory: 10 runs, 0.0 corrections per 100 runs, $0.10 per run"), "{report}");
    assert!(report.contains("Without memory: 5 runs, 100.0 corrections per 100 runs, $0.20 per run"), "{report}");
}

#[test]
fn too_few_held_out_runs_say_so() {
    let mut card = Scorecard::default();
    card.record(false, &outcome(0, 0.1));
    card.record(true, &outcome(0, 0.1));
    assert!(card.report().contains("Not enough runs without memory yet"), "{}", card.report());
}
