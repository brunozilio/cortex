use super::*;
use crate::trace::Record;
use crate::{EngineEvent, ToolCall};

#[test]
fn a_prompt_sent_with_files_is_shown_again_as_the_user_wrote_it() {
    let prompt = format!("sum it\n\nby month{ATTACHED}- /kept/7/sales.csv\n- /kept/8/q1.pdf");
    let mut transcript = Transcript::default();
    transcript.replay(&[Record::Prompted { at_ms: 1, prompt }]);

    assert_eq!(
        transcript.entries[..2],
        [
            Entry::Attached { names: vec!["sales.csv".into(), "q1.pdf".into()] },
            Entry::User("sum it\n\nby month".into())
        ]
    );
}

const ATTACHED: &str = "\n\nThe user attached these files; read them as needed:\n";

#[test]
fn a_prompt_that_only_looks_like_one_sent_with_files_is_shown_whole() {
    for prompt in ["just this\n- not a file".to_owned(), format!("quote it{ATTACHED}")] {
        let mut transcript = Transcript::default();
        transcript.replay(&[Record::Prompted { at_ms: 1, prompt: prompt.clone() }]);
        assert_eq!(transcript.entries[0], Entry::User(prompt));
    }
}

#[test]
fn what_the_user_saw_they_sent_is_what_the_history_shows_not_what_the_agent_was_told() {
    let records = [
        Record::Shown { text: "read @notes.txt".into(), attached: vec!["chart.png".into()] },
        Record::Prompted { at_ms: 1, prompt: "read @notes.txt\n\n<file path=\"notes.txt\">\nalpha\n</file>".into() },
        Record::Event { at_ms: 2, event: EngineEvent::Completed { duration_ms: 1, cost_usd: None } },
        Record::Prompted { at_ms: 3, prompt: "thanks".into() },
    ];
    let mut transcript = Transcript::default();
    transcript.replay(&records);

    assert_eq!(
        transcript.entries,
        [
            Entry::Attached { names: vec!["chart.png".into()] },
            Entry::User("read @notes.txt".into()),
            Entry::Finished { duration_ms: 1, cost_usd: None },
            Entry::User("thanks".into()),
        ]
    );
}

#[test]
fn a_kept_history_is_shown_again_as_the_conversation_it_was() {
    let records = [
        Record::Prompted { at_ms: 1, prompt: "read the notes".into() },
        Record::Event { at_ms: 2, event: EngineEvent::ToolStarted(ToolCall::new("t1", "Read", "notes.txt")) },
        Record::Event {
            at_ms: 3,
            event: EngineEvent::ToolResult { id: "t1".into(), output: "alpha".into(), failed: false },
        },
        Record::Event { at_ms: 4, event: EngineEvent::TextDelta("It says alpha.".into()) },
        Record::Event { at_ms: 5, event: EngineEvent::Completed { duration_ms: 4000, cost_usd: None } },
        Record::Prompted { at_ms: 9, prompt: "thanks".into() },
        Record::Event { at_ms: 10, event: EngineEvent::TextDelta("Any time.".into()) },
    ];
    let mut transcript = Transcript::default();
    transcript.replay(&records);

    assert_eq!(
        transcript.entries,
        [
            Entry::User("read the notes".into()),
            Entry::Tool {
                id: "t1".into(),
                name: "Read".into(),
                summary: "notes.txt".into(),
                state: ToolState::Done,
                output: Some("alpha".into()),
            },
            Entry::Assistant("It says alpha.".into()),
            Entry::Finished { duration_ms: 4_000, cost_usd: None },
            Entry::User("thanks".into()),
            Entry::Assistant("Any time.".into()),
        ]
    );
    assert_eq!(transcript.status(), SessionStatus::Cancelled, "a run the log never saw end did not go on");
    assert_eq!(transcript.activity, None);
}

#[test]
fn a_history_that_ended_well_is_a_finished_session() {
    let mut transcript = Transcript::default();
    transcript.replay(&[
        Record::Prompted { at_ms: 1, prompt: "hi".into() },
        Record::Event { at_ms: 2, event: EngineEvent::Completed { duration_ms: 1, cost_usd: None } },
        Record::Restarted,
    ]);
    assert_eq!(transcript.status(), SessionStatus::Done);
}

fn turn(at_ms: u64, prompt: &str, answer: &str) -> Vec<Record> {
    vec![
        Record::Prompted { at_ms, prompt: prompt.into() },
        Record::Event { at_ms: at_ms + 1, event: EngineEvent::TextDelta(answer.into()) },
        Record::Event { at_ms: at_ms + 2, event: EngineEvent::Completed { duration_ms: 1, cost_usd: None } },
    ]
}

#[test]
fn following_a_conversation_in_pieces_shows_what_replaying_it_at_once_shows() {
    let records: Vec<Record> = turn(1, "one", "First.").into_iter().chain(turn(10, "two", "Second.")).collect();
    let mut whole = Transcript::default();
    whole.replay(&records);

    let mut followed = Transcript::default();
    let used = followed.follow(&records[..4]);
    let used = used + followed.follow(&records[used..]);

    assert_eq!(used, records.len());
    assert_eq!(followed.entries, whole.entries);
}

#[test]
fn a_turn_still_going_elsewhere_stays_open_while_followed() {
    let records = &turn(1, "one", "Working on it")[..2];
    let mut transcript = Transcript::default();

    transcript.follow(records);

    assert_eq!(transcript.status(), SessionStatus::Running);
    assert!(transcript.entries.contains(&Entry::User("one".into())));
}

#[test]
fn what_the_user_saw_waits_for_its_prompt_before_it_is_followed() {
    let shown = Record::Shown { text: "read @notes.txt".into(), attached: Vec::new() };
    let prompted =
        Record::Prompted { at_ms: 1, prompt: "read @notes.txt\n\n<file path=\"notes.txt\">\nalpha\n</file>".into() };
    let mut transcript = Transcript::default();

    assert_eq!(transcript.follow(std::slice::from_ref(&shown)), 0, "kept for the next look");
    assert_eq!(transcript.follow(&[shown, prompted]), 2);
    assert_eq!(transcript.entries.last(), Some(&Entry::User("read @notes.txt".into())));
}
