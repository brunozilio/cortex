use domain::EngineEvent;
use domain::conversation::{Entry, Transcript};
use domain::trace::Record;

use super::*;

fn id(text: &str) -> SessionId {
    SessionId(text.into())
}

fn turn(at_ms: u64, prompt: &str, answer: &str) -> Vec<Record> {
    vec![
        Record::Prompted { at_ms, prompt: prompt.into() },
        Record::Event { at_ms: at_ms + 1, event: EngineEvent::TextDelta(answer.into()) },
        Record::Event { at_ms: at_ms + 2, event: EngineEvent::Completed { duration_ms: 1, cost_usd: None } },
    ]
}

fn settled(watch: &mut Watch, session: &SessionId, now: Instant, records: usize) {
    assert_eq!(watch.look(Some(session), false, now), Look::Settle(session.clone()));
    watch.settled(session, 100, records);
}

#[test]
fn without_a_conversation_there_is_nothing_to_watch() {
    assert_eq!(Watch::default().look(None, false, Instant::now()), Look::Nothing);
}

#[test]
fn a_conversation_is_first_taken_as_it_is_then_checked_now_and_then() {
    let (mut watch, session, now) = (Watch::default(), id("s"), Instant::now());
    settled(&mut watch, &session, now, 3);

    assert_eq!(watch.look(Some(&session), false, now), Look::Nothing, "too soon");
    assert_eq!(watch.look(Some(&session), false, now + EVERY), Look::Check(session.clone(), 100));
}

#[test]
fn what_this_window_writes_itself_is_never_shown_twice() {
    let (mut watch, session, now) = (Watch::default(), id("s"), Instant::now());
    settled(&mut watch, &session, now, 3);

    assert_eq!(watch.look(Some(&session), true, now + EVERY), Look::Nothing, "answering here");
    assert_eq!(
        watch.look(Some(&session), false, now + EVERY * 2),
        Look::Settle(session.clone()),
        "taken as it is again"
    );
}

#[test]
fn what_another_window_added_is_shown_and_nothing_before_it() {
    let (mut watch, session, now) = (Watch::default(), id("s"), Instant::now());
    let records: Vec<Record> = turn(1, "one", "First.").into_iter().chain(turn(10, "two", "Second.")).collect();
    settled(&mut watch, &session, now, 3);
    let mut transcript = Transcript::default();

    assert_eq!(watch.look(Some(&session), false, now + EVERY), Look::Check(session.clone(), 100));
    watch.arrived(&mut transcript, &session, 200, &records, false);

    assert!(transcript.entries.contains(&Entry::User("two".into())));
    assert!(!transcript.entries.contains(&Entry::User("one".into())));
    assert_eq!(watch.look(Some(&session), false, now + EVERY * 2), Look::Check(session.clone(), 200));
}

#[test]
fn a_turn_started_here_meanwhile_leaves_the_news_for_later() {
    let (mut watch, session, now) = (Watch::default(), id("s"), Instant::now());
    settled(&mut watch, &session, now, 0);
    let mut transcript = Transcript::default();
    watch.look(Some(&session), false, now + EVERY);

    watch.arrived(&mut transcript, &session, 200, &turn(1, "one", "First."), true);

    assert!(transcript.entries.is_empty());
    assert_eq!(watch.look(Some(&session), false, now + EVERY * 2), Look::Settle(session.clone()));
}

#[test]
fn a_history_cut_short_elsewhere_is_taken_as_it_is_again() {
    let (mut watch, session, now) = (Watch::default(), id("s"), Instant::now());
    settled(&mut watch, &session, now, 6);
    let mut transcript = Transcript::default();
    watch.look(Some(&session), false, now + EVERY);

    watch.arrived(&mut transcript, &session, 50, &turn(1, "one", "First."), false);

    assert!(transcript.entries.is_empty());
    assert_eq!(watch.look(Some(&session), false, now + EVERY * 2), Look::Settle(session.clone()));
}

#[test]
fn switching_conversations_starts_watching_the_new_one_afresh() {
    let (mut watch, now) = (Watch::default(), Instant::now());
    settled(&mut watch, &id("a"), now, 3);

    assert_eq!(watch.look(Some(&id("b")), false, now + EVERY), Look::Settle(id("b")));
}

#[test]
fn an_unchanged_conversation_lets_the_next_check_come() {
    let (mut watch, session, now) = (Watch::default(), id("s"), Instant::now());
    settled(&mut watch, &session, now, 3);
    watch.look(Some(&session), false, now + EVERY);

    watch.unchanged();

    assert_eq!(watch.look(Some(&session), false, now + EVERY * 2), Look::Check(session, 100));
}
