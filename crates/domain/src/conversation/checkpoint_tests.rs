use super::*;
use crate::{EngineEvent, Rewound};

fn two_turns() -> Transcript {
    let mut transcript = Transcript::recording();
    for (index, prompt) in ["first", "second"].into_iter().enumerate() {
        transcript.push(Entry::User(prompt.into()));
        transcript.begin();
        transcript.apply(&EngineEvent::Checkpoint(u32::try_from(index).unwrap_or_default()));
        transcript.apply(&EngineEvent::TextDelta(format!("answer to {prompt}")));
        transcript.apply(&EngineEvent::Completed { duration_ms: 1, cost_usd: None });
    }
    transcript.take_changes();
    transcript
}

#[test]
fn each_prompt_of_a_recorded_conversation_is_a_place_to_go_back_to() {
    let transcript = two_turns();
    assert_eq!(transcript.checkpoints(), [Checkpoint { index: 0, entry: 0 }, Checkpoint { index: 1, entry: 3 }]);
}

#[test]
fn going_back_drops_what_came_after_and_hands_the_prompt_back() {
    let mut transcript = two_turns();
    let rewound = Rewound {
        checkpoint: 1,
        prompt: "second".into(),
        restored: vec!["a.rs".into(), "b.rs".into()],
        conversation: true,
    };
    transcript.apply(&EngineEvent::Rewound(rewound));

    assert_eq!(transcript.entries.len(), 4);
    assert_eq!(transcript.entries[3], Entry::Rewound { restored: 2, conversation: true });
    assert_eq!(transcript.checkpoints(), [Checkpoint { index: 0, entry: 0 }]);
    assert_eq!(transcript.take_draft().as_deref(), Some("second"));
    assert_eq!(transcript.take_draft(), None);
    let changes = transcript.take_changes();
    assert!(changes.contains(&Change::Cut(3)) && changes.contains(&Change::Checkpoints), "{changes:?}");
    assert!(changes.contains(&Change::Draft));
}

#[test]
fn a_branch_starts_from_before_the_prompt_it_was_taken_at() {
    let mut transcript = two_turns();
    transcript.apply(&EngineEvent::Branched {
        checkpoint: 0,
        prompt: "first".into(),
        from: crate::SessionId("a".into()),
    });

    assert_eq!(transcript.entries, [Entry::Branched]);
    assert!(transcript.checkpoints().is_empty());
    assert_eq!(transcript.take_draft().as_deref(), Some("first"));
}

#[test]
fn going_back_to_a_place_the_conversation_does_not_know_changes_nothing() {
    let mut transcript = two_turns();
    let before = transcript.entries.clone();
    let rewound = Rewound { checkpoint: 9, prompt: "x".into(), restored: Vec::new(), conversation: true };
    transcript.apply(&EngineEvent::Rewound(rewound));
    assert_eq!(transcript.entries, before);
    assert_eq!(transcript.take_draft(), None);
}

#[test]
fn a_conversation_nobody_records_keeps_no_places() {
    let mut transcript = Transcript::default();
    transcript.push(Entry::User("first".into()));
    transcript.apply(&EngineEvent::Checkpoint(0));
    assert!(transcript.checkpoints().is_empty());
}
