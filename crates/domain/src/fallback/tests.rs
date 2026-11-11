use super::*;

fn chain(models: &[&str]) -> Fallback {
    Fallback::new(models.iter().map(|model| (*model).to_owned()).collect())
}

#[test]
fn an_overloaded_model_goes_down_the_chain_in_order() {
    let mut fallback = chain(&["orbit:a", "acme:b"]);
    assert_eq!(fallback.next("nova:x", Failure::Overloaded, Verdict::Pass), Some("orbit:a".into()));
    assert_eq!(fallback.next("orbit:a", Failure::Overloaded, Verdict::Pass), Some("acme:b".into()));
    assert_eq!(fallback.next("acme:b", Failure::Overloaded, Verdict::Pass), None);
    assert_eq!(fallback.tried(), ["nova:x", "orbit:a", "acme:b"]);
}

#[test]
fn the_chain_alone_answers_only_overload() {
    let mut fallback = chain(&["orbit:a"]);
    for failure in [Failure::Unauthorized, Failure::Network, Failure::Provider, Failure::TooLong] {
        assert_eq!(fallback.next("nova:x", failure, Verdict::Pass), None, "{failure:?}");
    }
}

#[test]
fn a_hook_forwards_any_failure_to_the_model_it_names() {
    let mut fallback = chain(&[]);
    let forward = Verdict::Forward("orbit:a".into());
    assert_eq!(fallback.next("nova:x", Failure::Unauthorized, forward), Some("orbit:a".into()));
}

#[test]
fn a_hook_overrides_the_chain() {
    let mut fallback = chain(&["orbit:a"]);
    let forward = Verdict::Forward("acme:b".into());
    assert_eq!(fallback.next("nova:x", Failure::Overloaded, forward), Some("acme:b".into()));
    let mut stopped = chain(&["orbit:a"]);
    assert_eq!(stopped.next("nova:x", Failure::Overloaded, Verdict::Stop), None);
}

#[test]
fn a_model_that_already_failed_is_never_tried_again_in_the_same_streak() {
    let mut fallback = chain(&["nova:x", "orbit:a"]);
    assert_eq!(fallback.next("nova:x", Failure::Overloaded, Verdict::Pass), Some("orbit:a".into()));
    let back = Verdict::Forward("nova:x".into());
    assert_eq!(fallback.next("orbit:a", Failure::Network, back), None);
    let same = Verdict::Forward("orbit:a".into());
    assert_eq!(chain(&[]).next("orbit:a", Failure::Network, same), None);
}

#[test]
fn a_success_ends_the_streak_so_a_later_failure_may_switch_again() {
    let mut fallback = chain(&["orbit:a"]);
    assert_eq!(fallback.next("nova:x", Failure::Overloaded, Verdict::Pass), Some("orbit:a".into()));
    fallback.succeeded();
    assert!(fallback.tried().is_empty());
    let back = Verdict::Forward("nova:x".into());
    assert_eq!(fallback.next("orbit:a", Failure::Overloaded, back), Some("nova:x".into()));
}

#[test]
fn a_streak_stops_after_the_most_switches() {
    let mut fallback = chain(&[]);
    let mut current = String::from("m0");
    let mut switches = 0;
    while let Some(next) = fallback.next(&current, Failure::Overloaded, Verdict::Forward(format!("m{}", switches + 1)))
    {
        current = next;
        switches += 1;
    }
    assert_eq!(switches, MAX_SWITCHES);
}

#[test]
fn failures_have_stable_labels_for_hooks() {
    let labels: Vec<_> =
        [Failure::Overloaded, Failure::Unauthorized, Failure::Network, Failure::Provider, Failure::TooLong]
            .map(Failure::label)
            .into();
    assert_eq!(labels, ["overloaded", "unauthorized", "network", "provider", "too_long"]);
}
