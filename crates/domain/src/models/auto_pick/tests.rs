use super::*;

#[test]
fn a_bare_go_ahead_keeps_the_session_s_model_at_low_effort_without_asking() {
    for prompt in ["ok", "Continue", "sim", "go on!", "  yes  ", "continua"] {
        assert_eq!(settled(prompt), Some(Pick { tier: Tier::Session, effort: Effort::Low }), "{prompt}");
    }
    assert_eq!(settled("ok, now refactor the parser"), None, "anything more is judged");
}

#[test]
fn the_judge_names_a_tier_and_an_effort() {
    assert_eq!(
        Pick::parse(r#"{"tier":"small","effort":"low"}"#),
        Some(Pick { tier: Tier::Small, effort: Effort::Low })
    );
    assert_eq!(
        Pick::parse("```json\n{\"tier\":\"large\",\"effort\":\"high\"}\n```"),
        Some(Pick { tier: Tier::Session, effort: Effort::High })
    );
    assert_eq!(Pick::parse(r#"{"tier":"large","effort":"max"}"#), None, "only low, medium and high");
    assert_eq!(Pick::parse("I think medium"), None);
}

#[test]
fn without_an_answer_the_run_goes_on_as_configured() {
    assert_eq!(Pick::FALLBACK, Pick { tier: Tier::Session, effort: Effort::Medium });
}
