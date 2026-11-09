use superfik_core::{TurnResponse, UserMessage};

use super::*;

#[test]
fn finished_turns_keep_their_own_duration_while_another_runs() {
    let mut turn = Turn::new(UserMessage { text: "hey".into(), images: vec![], branch_caption: String::new() });
    turn.response = TurnResponse {
        activity: Activity::Done,
        text: "Hi".into(),
        tools: vec![],
        elapsed_ms: 3_000,
        failure: None,
        usage: Vec::new(),
        structured: None,
    };
    assert_eq!(status(&Strings::default(), &turn, None, 20.0), ("Worked for 3s".into(), "1 step".into()));
    turn.response.activity = Activity::Writing;
    assert_eq!(status(&Strings::default(), &turn, Some(1.0), 20.0), ("Writing".into(), "19s".into()));
}
