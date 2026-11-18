use super::*;

fn minute_start() -> i64 {
    let now = Timestamp::now().as_second();
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
fn a_full_schedule_refuses_one_more_with_the_text_it_always_had() {
    let client = 7103;
    let start = minute_start();
    for at in 0..MOST {
        add(client, "wake up", When::At(start + 90 + i64::try_from(at).expect("small")), start).expect("room");
    }
    let refused = add(client, "one more", When::At(start + 90), start).expect_err("full");
    assert_eq!(refused, ScheduleError::Full);
    assert_eq!(String::from(refused), format!("At most {MOST} prompts can be scheduled; delete some with CronDelete."));
    let odd = add(client, "odd", When::Cron { expr: "not a cron".into(), recurring: true }, start);
    assert!(matches!(odd, Err(ScheduleError::When(_))), "{odd:?}");
    forget_client(client);
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

fn hours(east: i8) -> TimeZone {
    jiff::tz::Offset::from_hours(east).expect("an offset within a day").to_time_zone()
}

#[test]
fn a_schedule_is_shown_at_the_minute_on_the_user_s_clock() {
    let early_utc_morning = 1_793_849_400;
    assert_eq!(minute_in(&hours(0), early_utc_morning).as_deref(), Some("2026-11-05 03:30"));
    assert_eq!(minute_in(&hours(-8), early_utc_morning).as_deref(), Some("2026-11-04 19:30"));
    assert_eq!(minute_in(&hours(9), early_utc_morning + 59).as_deref(), Some("2026-11-05 12:30"));
    assert_eq!(minute_in(&hours(0), 0).as_deref(), Some("1970-01-01 00:00"));
}

#[test]
fn a_cron_expression_matches_the_moment_on_the_user_s_clock() {
    let early_utc_morning = 1_793_849_400;
    let utc = moment_in(&hours(0), early_utc_morning).expect("a moment");
    assert_eq!((utc.minute, utc.hour, utc.day, utc.month, utc.weekday), (30, 3, 5, 11, 4), "a Thursday");
    let west = moment_in(&hours(-8), early_utc_morning).expect("a moment");
    assert_eq!((west.minute, west.hour, west.day, west.month, west.weekday), (30, 19, 4, 11, 3));
    let sunday = moment_in(&hours(0), 1_794_096_000).expect("a moment");
    assert_eq!((sunday.hour, sunday.day, sunday.weekday), (0, 8, 0), "Sunday is day zero");
}

#[test]
fn scheduling_asks_first_and_listing_does_not() {
    use crate::tools::{Effect, Tool};
    let workspace = Workspace::new("/work/api");
    let acts = |tool: &dyn Tool| tool.effects(&Value::Null, &workspace).iter().any(Effect::acts);
    assert!(acts(&CronCreate), "a planted prompt would run unasked");
    assert!(acts(&ScheduleWakeup));
    assert!(!acts(&CronList));
    assert!(!acts(&CronDelete), "only this client's schedules can be removed");
}
