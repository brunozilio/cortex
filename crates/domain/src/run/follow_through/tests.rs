use super::*;

#[test]
fn only_an_answer_that_promises_more_work_is_worth_a_judgement() {
    for promise in [
        "Fixed the parser. Now I'll run the tests.",
        "Next, I will update the README.",
        "Let me check the other callers.",
        "Agora vou rodar os testes.",
    ] {
        assert!(worth_judging(promise, 0), "{promise}");
    }
    for settled in ["Done: the tests pass.", "Which database do you prefer?", ""] {
        assert!(!worth_judging(settled, 0), "{settled}");
    }
}

#[test]
fn the_agent_is_nudged_at_most_twice_per_request() {
    let promise = "Now I'll run the tests.";
    assert!(worth_judging(promise, MOST_NUDGES - 1));
    assert!(!worth_judging(promise, MOST_NUDGES));
}

#[test]
fn the_judge_answers_continue_with_a_nudge_done_or_waiting() {
    assert_eq!(
        Judgement::parse(r#"{"verdict":"continue","nudge":"Run cargo test."}"#),
        Some(Judgement::Continue("Run cargo test.".into()))
    );
    assert_eq!(Judgement::parse("```json\n{\"verdict\":\"done\"}\n```"), Some(Judgement::Done));
    assert_eq!(Judgement::parse(r#"{"verdict":"waiting"}"#), Some(Judgement::Waiting));
    assert_eq!(Judgement::parse(r#"{"verdict":"continue"}"#), None, "a nudge needs words");
    assert_eq!(Judgement::parse("sure!"), None);
}

#[test]
fn a_nudge_is_marked_so_later_answers_can_count_it() {
    let nudge = Judgement::Continue("Run cargo test.".into()).nudge().expect("nudge");
    assert!(nudge.starts_with(MARK));
    assert_eq!(nudges([nudge.as_str(), "fix the bug", nudge.as_str()]), 2);
    assert_eq!(Judgement::Done.nudge(), None);
}
