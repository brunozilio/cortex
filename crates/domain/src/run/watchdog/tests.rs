use super::*;

#[test]
fn a_loop_that_keeps_beating_is_never_stalled() {
    let mut dog = Watchdog::default();
    for at in (0..10_000).step_by(100) {
        dog.enter(at, "draw");
        assert_eq!(dog.check(at + 50), None);
    }
}

#[test]
fn a_step_that_runs_too_long_is_reported_once_with_its_name() {
    let mut dog = Watchdog::default();
    dog.enter(1_000, "messages");
    dog.enter(1_010, "draw");
    assert_eq!(dog.check(1_010 + STALL_MS - 1), None);
    assert_eq!(dog.check(1_010 + STALL_MS), Some(Stall { step: "draw".into(), for_ms: STALL_MS }));
    assert_eq!(dog.check(1_010 + STALL_MS + 500), None, "reported once");
}

#[test]
fn when_the_loop_comes_back_it_says_how_long_it_froze() {
    let mut dog = Watchdog::default();
    dog.enter(0, "input");
    assert!(dog.check(STALL_MS * 2).is_some());
    assert_eq!(dog.enter(3_500, "draw"), Some(Stall { step: "input".into(), for_ms: 3_500 }));
    assert_eq!(dog.enter(3_600, "draw"), None);
}

#[test]
fn waiting_on_purpose_is_not_a_freeze() {
    let mut dog = Watchdog::default();
    dog.enter(0, "draw");
    dog.pause();
    assert_eq!(dog.check(60_000), None);
    assert_eq!(dog.enter(60_000, "draw"), None);
}

#[test]
fn a_short_pause_is_not_called_a_freeze() {
    let mut dog = Watchdog::default();
    dog.enter(0, "draw");
    assert_eq!(dog.enter(STALL_MS - 1, "draw"), None);
}

#[test]
fn the_dog_names_the_step_it_is_in() {
    let mut dog = Watchdog::default();
    assert_eq!(dog.step(), "");
    dog.enter(0, "draw");
    assert_eq!(dog.step(), "draw");
}
