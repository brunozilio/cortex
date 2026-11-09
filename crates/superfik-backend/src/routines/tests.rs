use std::path::Path;

use superfik_core::routine::{Autonomy, Ending, MAX_REPORT_CHARS, RoutineDraft, RoutineId, Schedule};
use superfik_core::{EngineEvent, EngineKind, ModelOption, PermissionMode};

use super::*;

const NOW: i64 = 1_792_972_800;

fn project() -> &'static Path {
    Path::new("/repo")
}

fn draft(model: Option<&str>) -> RoutineDraft {
    RoutineDraft {
        name: "Upkeep".into(),
        brief: "Triage new Sentry issues".into(),
        schedule: Schedule::Every { minutes: 30 },
        autonomy: Autonomy::Draft,
        model: model.map(str::to_owned),
        connections: Vec::new(),
        answers: false,
    }
}

fn book_of(event: ServerEvent) -> RoutineBook {
    match event {
        ServerEvent::Routines { book, .. } => book,
        other => panic!("expected routines, got {other:?}"),
    }
}

fn with_one(routines: &Routines) {
    routines.edit(&Principal::LocalOwner, project(), &[RoutineEdit::Create { draft: draft(None) }], NOW);
}

#[test]
fn edits_are_kept_and_answered_with_the_book() {
    let routines = Routines::in_memory();
    with_one(&routines);
    let book = book_of(routines.load(&Principal::LocalOwner, project()));
    assert_eq!(book.routines.len(), 1);
    assert_eq!(book.routines[0].name, "Upkeep");
}

#[test]
fn due_lists_firings_with_their_owner_and_project() {
    let routines = Routines::in_memory();
    with_one(&routines);
    assert!(routines.due(NOW).is_empty());
    let due = routines.due(NOW + 1_800);
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].owner, Principal::LocalOwner);
    assert_eq!(due[0].project, project());
}

#[test]
fn waking_reads_stored_books_and_settles_lost_firings() {
    let store: Arc<dyn RoutineStore> = Arc::new(InMemoryRoutineStore::default());
    let before = Routines::new(Arc::clone(&store));
    with_one(&before);
    let firing = before.due(NOW + 1_800).remove(0);
    let run = RunId::new();
    before.started(&firing, run, None, NOW + 1_800);

    let after = Routines::new(store);
    after.wake(NOW + 3_600);
    let book = book_of(after.load(&Principal::LocalOwner, project()));
    assert_eq!(book.report(run).and_then(|r| r.ending.clone()), Some(Ending::Cancelled));
    assert_eq!(after.due(NOW + 3_600).len(), 1, "woken books are scheduled");
}

#[test]
fn a_firing_runs_the_routine_prompt_in_its_project_with_its_autonomy() {
    let routines = Routines::in_memory();
    routines.edit(&Principal::LocalOwner, project(), &[RoutineEdit::Create { draft: draft(Some("sonnet")) }], NOW);
    let firing = routines.due(NOW + 1_800).remove(0);
    let opus = ModelOption::new(EngineKind::Native, "opus", "Opus");
    let sonnet = ModelOption::new(EngineKind::Native, "sonnet", "Sonnet");

    let picked = request(&firing, &[opus.clone(), sonnet.clone()], None, superfik_core::routine::Cause::Schedule)
        .unwrap_or_else(|| panic!("no request"));
    assert_eq!(picked.cwd, project());
    assert_eq!(picked.model, sonnet);
    assert_eq!(picked.permission, PermissionMode::Plan);
    assert_eq!(picked.prompt, firing.routine.prompt(None, superfik_core::routine::Cause::Schedule));
    assert!(picked.memory.is_some());

    let fallback = request(&firing, std::slice::from_ref(&opus), None, superfik_core::routine::Cause::Schedule)
        .unwrap_or_else(|| panic!("no request"));
    assert_eq!(fallback.model, opus);
    assert!(request(&firing, &[], None, superfik_core::routine::Cause::Schedule).is_none());
}

#[test]
fn a_firing_carries_the_routines_accounts_and_names_its_branch() {
    let sentry = superfik_core::Connection {
        id: superfik_core::ConnectionId::new(),
        plugin: superfik_core::PluginId("sentry".into()),
        plugin_name: "Sentry".into(),
        label: "acme".into(),
        icon: String::new(),
    };
    let routines = Routines::in_memory();
    let with = RoutineDraft { connections: vec![sentry.clone()], ..draft(None) };
    routines.edit(&Principal::LocalOwner, project(), &[RoutineEdit::Create { draft: with }], NOW);
    let firing = routines.due(NOW + 1_800).remove(0);
    let model = ModelOption::new(EngineKind::Native, "opus", "Opus");
    let request = request(&firing, &[model], Some("routine/upkeep"), superfik_core::routine::Cause::Schedule)
        .unwrap_or_else(|| panic!("no request"));
    assert_eq!(request.connections, vec![sentry]);
    assert!(request.prompt.contains("`routine/upkeep`"));
}

#[test]
fn only_routines_that_may_edit_get_a_worktree_under_the_root() {
    let routines = Routines::in_memory();
    let editing = RoutineDraft { autonomy: Autonomy::Bounded, ..draft(None) };
    routines.edit(&Principal::LocalOwner, project(), &[RoutineEdit::Create { draft: draft(None) }], NOW);
    routines.edit(&Principal::LocalOwner, project(), &[RoutineEdit::Create { draft: editing }], NOW);
    let due = routines.due(NOW + 1_800);
    let root = Path::new("/data/worktrees");
    let places: Vec<_> = due.iter().map(|firing| place(firing, root, NOW + 1_800)).collect();
    assert!(places.iter().any(Option::is_none));
    let (branch, path) = places.into_iter().flatten().next().unwrap_or_else(|| panic!("no place"));
    assert_eq!(branch, "routine/upkeep-20261026-0030");
    assert_eq!(path, root.join("-repo").join("routine-upkeep-20261026-0030"));
}

#[test]
fn a_trace_keeps_the_reply_and_records_the_ending() {
    let routines = Routines::in_memory();
    with_one(&routines);
    let firing = routines.due(NOW + 1_800).remove(0);
    let run = RunId::new();
    routines.started(&firing, run, None, NOW + 1_800);
    let trace = Trace::new(routines.clone(), &firing, None);

    assert!(trace.observe(run, &EngineEvent::TextDelta("Two new issues. ".into()), NOW).is_none());
    assert!(trace.observe(run, &EngineEvent::TextDelta("Both known.".into()), NOW).is_none());
    let done = EngineEvent::Completed { duration_ms: 5, cost_usd: Some(0.25) };
    let book = book_of(trace.observe(run, &done, NOW + 1_900).unwrap_or_else(|| panic!("no book")));

    let report = book.report(run).unwrap_or_else(|| panic!("no report"));
    assert_eq!(report.ending, Some(Ending::Completed));
    assert_eq!(report.text, "Two new issues. Both known.");
    assert!((report.cost_usd - 0.25).abs() < f64::EPSILON);
    assert_eq!(book.routines[0].id, RoutineId(0));
}

#[test]
fn a_long_reply_keeps_its_tail() {
    let mut reply = "é".repeat(MAX_REPORT_CHARS + 10);
    reply.push_str("end");
    firing::keep_tail(&mut reply);
    assert_eq!(reply.chars().count(), MAX_REPORT_CHARS);
    assert!(reply.ends_with("end"));
}
