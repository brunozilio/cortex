use super::*;

#[test]
fn nothing_changed_means_no_new_report() {
    assert!(!Refresh::default().start(Instant::now()));
}

#[test]
fn a_change_asks_for_one_new_report() {
    let mut refresh = Refresh::default();
    let now = Instant::now();
    refresh.changed();

    assert!(refresh.start(now));
    refresh.done();
    assert!(!refresh.start(now + GAP), "the change was already reported");
}

#[test]
fn changes_while_one_is_being_made_wait_for_it() {
    let mut refresh = Refresh::default();
    let now = Instant::now();
    refresh.changed();
    assert!(refresh.start(now));

    refresh.changed();
    assert!(!refresh.start(now + GAP), "one report at a time");
    refresh.done();
    assert!(refresh.start(now + GAP));
}

#[test]
fn reports_come_at_most_once_per_gap_and_the_last_change_is_not_lost() {
    let mut refresh = Refresh::default();
    let now = Instant::now();
    refresh.changed();
    assert!(refresh.start(now));
    refresh.done();

    refresh.changed();
    assert!(!refresh.start(now + GAP / 2), "too soon after the last one");
    assert!(refresh.start(now + GAP), "made once the gap has passed");
}
