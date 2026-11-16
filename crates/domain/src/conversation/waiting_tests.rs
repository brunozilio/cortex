use super::*;
use crate::{EngineEvent, FailureKind};

#[test]
fn what_was_said_to_a_running_agent_waits_in_line_until_the_agent_takes_it() {
    let mut transcript = Transcript::recording();
    transcript.steered("then run the tests");
    transcript.steered("and open the PR");
    assert_eq!(transcript.queued(), ["then run the tests", "and open the PR"]);
    assert_eq!(transcript.take_changes(), [Change::Queue]);

    transcript.apply(&EngineEvent::Steered("then run the tests".into()));
    assert_eq!(transcript.queued(), ["and open the PR"]);
    assert_eq!(transcript.entries, [Entry::User("then run the tests".into())]);
}

#[test]
fn what_the_agent_never_took_goes_back_to_the_user_to_send_again() {
    let mut transcript = Transcript::recording();
    transcript.steered("then run the tests");
    transcript.steered("and open the PR");
    transcript.take_changes();

    transcript.apply(&EngineEvent::Unsteered(vec!["then run the tests".into(), "and open the PR".into()]));

    assert!(transcript.queued().is_empty());
    assert_eq!(transcript.take_draft().as_deref(), Some("then run the tests\n\nand open the PR"));
    let changes = transcript.take_changes();
    assert!(changes.contains(&Change::Queue) && changes.contains(&Change::Draft), "{changes:?}");
}

#[test]
fn a_run_the_provider_dropped_is_owed_one_more_try_and_only_one() {
    let mut transcript = Transcript::recording();
    let dropped = |transcript: &mut Transcript, kind: FailureKind| {
        transcript.begin();
        transcript.apply(&EngineEvent::Failure(kind));
        transcript.apply(&EngineEvent::Failed("no answer".into()));
    };

    dropped(&mut transcript, FailureKind::Other);
    assert!(!transcript.owes_retry(), "only when the provider was unavailable");

    dropped(&mut transcript, FailureKind::Unavailable);
    assert!(transcript.owes_retry());
    assert!(!transcript.owes_retry(), "asked once");
    dropped(&mut transcript, FailureKind::Unavailable);
    assert!(!transcript.owes_retry(), "the second failure in a row is the user's to retry");

    transcript.fresh();
    dropped(&mut transcript, FailureKind::Unavailable);
    assert!(transcript.owes_retry(), "a prompt the user wrote starts over");
}
