use super::*;

const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;
const MONDAY: i64 = 1_792_972_800;
const BRT: i32 = -3 * 3_600;

fn draft(name: &str, schedule: Schedule) -> RoutineDraft {
    RoutineDraft {
        name: name.into(),
        brief: "Triage new Sentry issues".into(),
        schedule,
        autonomy: Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    }
}

fn book_with(schedule: Schedule, now: i64) -> RoutineBook {
    let mut book = RoutineBook::default();
    book.apply(&[RoutineEdit::Create { draft: draft("Upkeep", schedule) }], now);
    book
}

#[test]
fn the_reference_monday_is_a_monday() {
    assert_eq!(crate::days::weekday(crate::days::local_day(MONDAY, 0)), 0);
}

#[test]
fn every_counts_from_the_last_firing_with_a_floor() {
    assert_eq!(Schedule::Every { minutes: 60 }.next_after(MONDAY), MONDAY + HOUR);
    assert_eq!(Schedule::Every { minutes: 1 }.next_after(MONDAY), MONDAY + 5 * 60);
}

#[test]
fn daily_fires_at_the_local_minute_today_or_tomorrow() {
    let nine = Schedule::Daily { minute: 9 * 60, utc_offset: BRT };
    assert_eq!(nine.next_after(MONDAY + 6 * HOUR), MONDAY + 12 * HOUR);
    assert_eq!(nine.next_after(MONDAY + 12 * HOUR), MONDAY + DAY + 12 * HOUR);
    assert_eq!(nine.next_after(MONDAY + 13 * HOUR), MONDAY + DAY + 12 * HOUR);
}

#[test]
fn daily_near_midnight_uses_the_local_day() {
    let late = Schedule::Daily { minute: 23 * 60, utc_offset: BRT };
    assert_eq!(late.next_after(MONDAY + HOUR), MONDAY + 2 * HOUR);
}

#[test]
fn weekly_fires_on_its_weekday() {
    let friday = Schedule::Weekly { weekday: 4, minute: 18 * 60, utc_offset: 0 };
    assert_eq!(friday.next_after(MONDAY), MONDAY + 4 * DAY + 18 * HOUR);
    assert_eq!(friday.next_after(MONDAY + 4 * DAY + 18 * HOUR), MONDAY + 11 * DAY + 18 * HOUR);
    let monday = Schedule::Weekly { weekday: 0, minute: 8 * 60, utc_offset: 0 };
    assert_eq!(monday.next_after(MONDAY + 10 * HOUR), MONDAY + 7 * DAY + 8 * HOUR);
}

#[test]
fn a_new_routine_is_enabled_and_scheduled() {
    let book = book_with(Schedule::Every { minutes: 30 }, MONDAY);
    let routine = &book.routines[0];
    assert_eq!(routine.id, RoutineId(0));
    assert!(routine.enabled);
    assert_eq!(routine.next_run, Some(MONDAY + 30 * 60));
}

#[test]
fn drafts_without_a_name_or_brief_are_ignored() {
    let mut book = RoutineBook::default();
    let mut blank = draft("  ", Schedule::Every { minutes: 30 });
    let outcome = book.apply(&[RoutineEdit::Create { draft: blank.clone() }], MONDAY);
    blank.name = "Named".into();
    blank.brief = " ".into();
    book.apply(&[RoutineEdit::Create { draft: blank }], MONDAY);
    assert!(outcome.created.is_empty());
    assert!(book.routines.is_empty());
}

#[test]
fn due_routines_fire_once_and_reschedule_from_the_firing() {
    let mut book = book_with(Schedule::Every { minutes: 30 }, MONDAY);
    assert!(book.due(MONDAY + 29 * 60).is_empty());
    let late = MONDAY + 5 * HOUR;
    assert_eq!(book.due(late), vec![RoutineId(0)]);
    let run = RunId::new();
    assert!(book.start(RoutineId(0), run, None, late));
    assert_eq!(book.routines[0].next_run, Some(late + 30 * 60));
    assert_eq!(book.routines[0].last_run, Some(late));
    assert!(book.report(run).is_some_and(Report::is_running));
}

#[test]
fn a_running_firing_holds_back_the_next() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    book.start(RoutineId(0), RunId::new(), None, MONDAY + 300);
    assert!(book.due(MONDAY + 3_600).is_empty());
}

#[test]
fn finishing_keeps_the_reply_and_counts_as_unread() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    let run = RunId::new();
    book.start(RoutineId(0), run, None, MONDAY + 300);
    assert_eq!(book.unread(), 0);
    assert!(book.finish(run, Ending::Completed, "Two new issues, both known.", 0.12, MONDAY + 400));
    assert!(!book.finish(run, Ending::Completed, "again", 0.0, MONDAY + 500));
    let report = book.report(run).cloned().unwrap_or_else(|| panic!("no report"));
    assert_eq!(report.text, "Two new issues, both known.");
    assert_eq!(report.finished, Some(MONDAY + 400));
    assert_eq!(book.unread(), 1);
    book.apply(&[RoutineEdit::Read { run }], MONDAY + 600);
    assert_eq!(book.unread(), 0);
}

#[test]
fn disabling_stops_firings_and_enabling_reschedules() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    book.apply(&[RoutineEdit::Enable { id: RoutineId(0), enabled: false }], MONDAY);
    assert!(book.due(MONDAY + DAY).is_empty());
    book.apply(&[RoutineEdit::Enable { id: RoutineId(0), enabled: true }], MONDAY + DAY);
    assert_eq!(book.routines[0].next_run, Some(MONDAY + DAY + 300));
}

#[test]
fn run_now_makes_it_due_at_once() {
    let mut book = book_with(Schedule::Weekly { weekday: 4, minute: 0, utc_offset: 0 }, MONDAY);
    book.apply(&[RoutineEdit::RunNow { id: RoutineId(0) }], MONDAY + 10);
    assert_eq!(book.due(MONDAY + 10), vec![RoutineId(0)]);
}

#[test]
fn revising_the_schedule_reschedules() {
    let mut book = book_with(Schedule::Every { minutes: 60 }, MONDAY);
    let outcome = book.apply(
        &[RoutineEdit::Revise { id: RoutineId(0), draft: draft("Upkeep", Schedule::Every { minutes: 10 }) }],
        MONDAY + 60,
    );
    assert_eq!(outcome.changed, vec![RoutineId(0)]);
    assert_eq!(book.routines[0].next_run, Some(MONDAY + 60 + 600));
    let refused =
        book.apply(&[RoutineEdit::Revise { id: RoutineId(9), draft: draft("x", Schedule::Every { minutes: 10 }) }], 0);
    assert_eq!(refused.refused, vec![RoutineId(9)]);
}

#[test]
fn deleting_drops_finished_reports_and_ids_are_not_reused() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    let run = RunId::new();
    book.start(RoutineId(0), run, None, MONDAY);
    book.finish(run, Ending::Completed, "ok", 0.0, MONDAY);
    book.apply(&[RoutineEdit::Delete { id: RoutineId(0) }], MONDAY);
    assert!(book.reports.is_empty());
    book.apply(&[RoutineEdit::Create { draft: draft("Again", Schedule::Every { minutes: 5 }) }], MONDAY);
    assert_eq!(book.routines[0].id, RoutineId(1));
}

#[test]
fn settling_cancels_what_a_restart_lost() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    let run = RunId::new();
    book.start(RoutineId(0), run, None, MONDAY);
    assert!(book.settle(MONDAY + 10));
    assert_eq!(book.report(run).and_then(|r| r.ending.clone()), Some(Ending::Cancelled));
    assert!(!book.settle(MONDAY + 20));
}

#[test]
fn reports_are_capped_newest_first() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    let mut last = RunId::new();
    for i in 0..(MAX_REPORTS as i64 + 5) {
        last = RunId::new();
        book.start(RoutineId(0), last, None, MONDAY + i);
        book.finish(last, Ending::Completed, "", 0.0, MONDAY + i);
    }
    assert_eq!(book.reports.len(), MAX_REPORTS);
    assert_eq!(book.reports[0].run, last);
}

#[test]
fn autonomy_maps_to_a_permission_mode_and_shapes_the_prompt() {
    assert_eq!(Autonomy::Draft.mode(), PermissionMode::Plan);
    assert_eq!(Autonomy::Bounded.mode(), PermissionMode::AcceptEdits);
    assert_eq!(Autonomy::Auto.mode(), PermissionMode::Bypass);
    let book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    let prompt = book.routines[0].prompt(None, Cause::Schedule);
    assert!(prompt.contains("\"Upkeep\", started on its schedule"));
    assert!(prompt.contains("only read"));
    assert!(!prompt.contains("worktree"));
    assert!(prompt.ends_with("Triage new Sentry issues"));
    assert!(book.routines[0].prompt(Some("routine/upkeep-x"), Cause::Schedule).contains("`routine/upkeep-x`"));
}

#[test]
fn only_routines_that_may_edit_work_apart_on_a_dated_branch() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    assert!(!book.routines[0].works_apart());
    book.routines[0].autonomy = Autonomy::Bounded;
    book.routines[0].name = "Production upkeep!".into();
    assert!(book.routines[0].works_apart());
    assert_eq!(book.routines[0].branch(MONDAY + 9 * HOUR + 5 * 60), "routine/production-upkeep-20261026-0905");
    book.routines[0].name = "Сводка".into();
    assert_eq!(book.routines[0].branch(MONDAY), "routine/0-20261026-0000");
}

#[test]
fn a_firing_keeps_its_branch_and_accounts_survive_a_revise() {
    let mut book = book_with(Schedule::Every { minutes: 5 }, MONDAY);
    let run = RunId::new();
    book.start(RoutineId(0), run, Some("routine/upkeep".into()), MONDAY);
    assert_eq!(book.report(run).and_then(|r| r.branch.clone()), Some("routine/upkeep".into()));
    let sentry = crate::Connection {
        id: crate::ConnectionId::new(),
        plugin: crate::PluginId("sentry".into()),
        plugin_name: "Sentry".into(),
        label: "acme".into(),
        icon: String::new(),
    };
    let revised = RoutineDraft { connections: vec![sentry.clone()], ..draft("Upkeep", Schedule::Every { minutes: 5 }) };
    book.apply(&[RoutineEdit::Revise { id: RoutineId(0), draft: revised }], MONDAY);
    assert_eq!(book.routines[0].connections, vec![sentry]);
}

#[test]
fn templates_write_complete_drafts_in_the_members_offset() {
    for template in Template::ALL {
        let draft = template.draft("Name".into(), BRT);
        assert!(!draft.brief.is_empty());
        assert!(!template.plugins().is_empty());
        match draft.schedule {
            Schedule::Daily { utc_offset, .. } | Schedule::Weekly { utc_offset, .. } => assert_eq!(utc_offset, BRT),
            Schedule::Every { .. } => {}
        }
    }
    assert_eq!(Template::ProductionUpkeep.draft("x".into(), 0).autonomy, Autonomy::Bounded);
    assert_eq!(Template::ReleaseNotes.draft("x".into(), 0).autonomy, Autonomy::Draft);
}

#[test]
fn a_book_round_trips_through_json() {
    let mut book = book_with(Schedule::Daily { minute: 540, utc_offset: BRT }, MONDAY);
    let run = RunId::new();
    book.start(RoutineId(0), run, None, MONDAY);
    book.finish(run, Ending::Failed("CLI missing".into()), "", 0.0, MONDAY);
    let json = serde_json::to_string(&book).unwrap_or_default();
    let back: RoutineBook = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(back, book);
}
