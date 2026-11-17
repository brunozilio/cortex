use super::*;

#[test]
fn each_strategy_has_a_name_it_is_typed_and_saved_under() {
    let names: Vec<_> = Balance::ALL.iter().map(|b| b.name()).collect();
    assert_eq!(names, ["off", "failover", "round-robin"]);
    assert_eq!(Balance::parse(" Round-Robin "), Some(Balance::RoundRobin));
    assert_eq!(Balance::parse("random"), None);
    assert_eq!(serde_json::to_value(Balance::RoundRobin).expect("json"), "round-robin");
}

#[test]
fn off_only_ever_uses_the_account_in_use() {
    assert_eq!(order(Balance::Off, 3, 1, 7), [1]);
}

#[test]
fn failover_starts_on_the_account_in_use_and_tries_the_rest_in_turn() {
    assert_eq!(order(Balance::Failover, 3, 1, 0), [1, 2, 0]);
    assert_eq!(order(Balance::Failover, 3, 1, 5), [1, 2, 0], "every run starts on the same account");
}

#[test]
fn round_robin_starts_each_run_on_the_next_account() {
    assert_eq!(order(Balance::RoundRobin, 3, 0, 0), [0, 1, 2]);
    assert_eq!(order(Balance::RoundRobin, 3, 0, 1), [1, 2, 0]);
    assert_eq!(order(Balance::RoundRobin, 3, 0, 4), [1, 2, 0]);
}

#[test]
fn a_lone_account_or_none_gives_what_there_is() {
    assert_eq!(order(Balance::RoundRobin, 1, 0, 9), [0]);
    assert!(order(Balance::Failover, 0, 0, 0).is_empty());
}

#[test]
fn a_busy_or_signed_out_account_hands_over_and_other_failures_do_not() {
    use crate::fallback::Failure;
    assert!(hands_over(Failure::Overloaded) && hands_over(Failure::Unauthorized));
    assert!(!hands_over(Failure::TooLong) && !hands_over(Failure::Provider) && !hands_over(Failure::Network));
}
