use super::*;

/// A Unix time at the start of a local minute.
fn minute_start() -> i64 {
    let now = Local::now().timestamp();
    now - now.rem_euclid(60)
}

#[test]
fn a_recurring_prompt_fires_at_each_match_and_a_one_shot_once() {
    let client = 7101;
    let start = minute_start();
    let every_minute =
        add(client, "check the build", When::Cron { expr: "* * * * *".into(), recurring: true }, start).expect("cron");
    assert_eq!(every_minute.next, start + 60);
    add(client, "wake up", When::At(start + 90), start).expect("wakeup");

    assert!(take_due(client, start + 59).is_empty(), "nothing is due yet");
    let due: Vec<String> = take_due(client, start + 60).into_iter().map(|due| due.prompt).collect();
    assert_eq!(due, ["check the build"]);
    let due: Vec<String> = take_due(client, start + 125).into_iter().map(|due| due.prompt).collect();
    assert_eq!(due, ["check the build", "wake up"], "missed minutes fire once");
    let listed = list(client);
    assert_eq!(listed.len(), 1, "the one-shot left: {listed:?}");
    assert_eq!(listed[0].next, start + 180);

    assert!(remove(client, &every_minute.id));
    assert!(list(client).is_empty());
    assert!(!remove(client, &every_minute.id));
}

#[test]
fn a_recurring_prompt_expires_after_a_week() {
    let client = 7102;
    let start = minute_start();
    add(client, "daily", When::Cron { expr: "* * * * *".into(), recurring: true }, start).expect("cron");
    let week = 7 * 24 * 3600;
    assert_eq!(take_due(client, start + week + 60).len(), 1, "fires one last time");
    assert!(list(client).is_empty());
}

#[test]
fn prompts_belong_to_their_client_until_it_leaves() {
    let start = minute_start();
    add(7103, "mine", When::At(start), start).expect("at");
    assert!(take_due(7104, start + 1).is_empty(), "another client's");
    add(7103, "also mine", When::At(start + 600), start).expect("at");
    forget_client(7103);
    assert!(list(7103).is_empty());
    assert!(add(7103, "x", When::Cron { expr: "nope".into(), recurring: true }, start).is_err());
}

#[test]
fn scheduling_asks_first_and_listing_does_not() {
    use crate::tools::Tool;
    assert_eq!(CronCreate.access(), crate::tools::Access::Executes, "a planted prompt would run unasked");
    assert_eq!(ScheduleWakeup.access(), crate::tools::Access::Executes);
    assert_eq!(CronList.access(), crate::tools::Access::Reads);
}
