use super::*;
use crate::PermissionMode;

#[test]
fn plan_verification_reads_the_bullet_section_up_to_the_next_item() {
    let plan = "\
- **Approach**: do it.
- **Verification**: run `cargo test`.
  Then launch the app and open settings.
- **Reuse**: none.";
    assert_eq!(plan_verification(plan).as_deref(), Some("run `cargo test`.\nThen launch the app and open settings."));
}

#[test]
fn plan_verification_reads_a_heading_section_up_to_the_next_heading() {
    let plan = "# Plan\n\n## Verification\n\n1. cargo test\n2. cargo clippy\n\n## Risks\nnone";
    assert_eq!(plan_verification(plan).as_deref(), Some("1. cargo test\n2. cargo clippy"));
}

#[test]
fn a_plan_without_the_section_has_none() {
    assert_eq!(plan_verification("- **Approach**: do it."), None);
    assert_eq!(plan_verification("## Verification\n\n## Next"), None);
}

#[test]
fn the_users_criteria_win_over_the_plans() {
    let user = Criteria::user("cargo test passes");
    let plan = Criteria::plan("run the app");
    assert_eq!(Criteria::resolve(Some(&user), Some(&plan)), Some(user.clone()));
    assert_eq!(Criteria::resolve(None, Some(&plan)), Some(plan));
    assert_eq!(Criteria::resolve(Some(&user), None), Some(user));
    assert_eq!(Criteria::resolve(None, None), None);
}

#[test]
fn a_verdict_is_read_from_the_judges_json_even_fenced() {
    let reply = "```json\n{\"passed\": false, \"score\": 1.7, \"checks\": [\
        {\"criterion\": \"tests pass\", \"passed\": false, \"evidence\": \"cargo test: 1 failed\"}]}\n```";
    let verdict = Verdict::parse(reply, 2).expect("verdict");
    assert!(!verdict.passed);
    assert_eq!(verdict.score, 1.0);
    assert_eq!(verdict.attempt, 2);
    assert_eq!(
        verdict.checks,
        vec![Check { criterion: "tests pass".into(), passed: false, evidence: "cargo test: 1 failed".into() }]
    );
}

#[test]
fn a_verdict_without_a_score_scores_its_passed_checks() {
    let reply = "Here it is: {\"passed\": true, \"checks\": [\
        {\"criterion\": \"a\", \"passed\": true}, {\"criterion\": \"b\", \"passed\": false}]}";
    assert_eq!(Verdict::parse(reply, 1).expect("verdict").score, 0.5);
}

#[test]
fn a_reply_without_a_verdict_is_an_error() {
    assert!(Verdict::parse("looks good to me", 1).is_err());
    assert!(Verdict::parse("{\"score\": 1}", 1).is_err());
}

#[test]
fn a_failed_verdict_is_retried_until_the_last_attempt() {
    let failed = |attempt| Verdict::parse(r#"{"passed":false}"#, attempt).expect("verdict");
    assert!(failed(1).should_retry());
    assert!(failed(MAX_ATTEMPTS - 1).should_retry());
    assert!(!failed(MAX_ATTEMPTS).should_retry());
    let mut passed = failed(1);
    passed.passed = true;
    assert!(!passed.should_retry());
}

#[test]
fn a_judge_that_could_not_grade_is_not_retried() {
    let verdict = Verdict::unavailable("provider down", 1);
    assert!(verdict.is_unavailable() && !verdict.passed);
    assert!(!verdict.should_retry());
}

#[test]
fn feedback_lists_only_the_failed_checks() {
    let verdict = Verdict::parse(
        "{\"passed\": false, \"checks\": [{\"criterion\": \"a\", \"passed\": true, \"evidence\": \"ok\"}, \
         {\"criterion\": \"b\", \"passed\": false, \"evidence\": \"missing\"}]}",
        1,
    )
    .expect("verdict");
    assert_eq!(verdict.feedback(), "Verification failed (attempt 1/3). Fix these and finish again:\n- b: missing");
}

#[test]
fn a_finished_plan_sets_the_criteria_for_the_runs_after_it() {
    let mut state = Verifications::default();
    assert_eq!(state.for_run(PermissionMode::Ask), None);
    state.plan_finished("- **Approach**: x\n- **Verification**: cargo test");
    assert_eq!(state.for_run(PermissionMode::Plan), None, "a planning run is never judged");
    assert_eq!(state.for_run(PermissionMode::Bypass), Some(Criteria::plan("cargo test")));
    state.plan_finished("A plan without the section");
    assert_eq!(state.current(), None, "the next plan replaces the last");
}

#[test]
fn the_users_criteria_stand_until_cleared() {
    let mut state = Verifications::default();
    state.plan_finished("## Verification\nrun the app");
    state.set_user(Some("  cargo test passes ".into()));
    assert_eq!(state.current(), Some(Criteria::user("cargo test passes")));
    state.set_user(None);
    assert_eq!(state.current(), Some(Criteria::plan("run the app")));
    state.set_user(Some("   ".into()));
    assert_eq!(state.current(), Some(Criteria::plan("run the app")), "blank criteria set nothing");
}
