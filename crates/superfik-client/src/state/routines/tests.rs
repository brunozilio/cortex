//! Reducer rules for the Routines screen: loading, the form, edits sent to
//! the backend, reports read and acted on.

use std::path::PathBuf;

use superfik_core::RunId;
use superfik_core::routine::{Autonomy, Ending, RoutineBook, RoutineEdit, RoutineId, Schedule};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::super::tests::commands;
use super::super::{Action, EditKey, Session, WallClock};
use super::*;

const CLOCK: WallClock = WallClock { unix_at_zero: 1_792_972_800, utc_offset_secs: -3 * 3_600 };

fn routine(s: &mut Session, action: RoutineAction) -> Vec<Effect> {
    s.apply(Action::Routines(action), 0.0)
}

fn edits(effects: &[Effect]) -> Vec<RoutineEdit> {
    commands(effects)
        .into_iter()
        .flat_map(|c| match c {
            ClientCommand::EditRoutines { edits, .. } => edits.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn book(run: RunId) -> RoutineBook {
    let mut book = RoutineBook::default();
    let draft = superfik_core::routine::RoutineDraft {
        name: "Upkeep".into(),
        brief: "Triage Sentry".into(),
        schedule: Schedule::Every { minutes: 60 },
        autonomy: Autonomy::Draft,
        model: None,
        connections: Vec::new(),
        answers: false,
    };
    book.apply(&[RoutineEdit::Create { draft }], 0);
    book.start(RoutineId(0), run, None, 10);
    book.finish(run, Ending::Completed, "Two new issues; one is a regression.", 0.1, 20);
    book
}

fn session(run: RunId) -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.clock = CLOCK;
    s.apply(Action::Server(ServerEvent::Routines { path: "/repo".into(), book: book(run) }), 0.0);
    s
}

fn type_in(s: &mut Session, text: &str) {
    routine(s, RoutineAction::Edit(EditKey::Insert(text.into())));
}

#[test]
fn opening_a_workspace_loads_its_routines() {
    let mut s = Session::default();
    let effects = s.apply(Action::OpenWorkspace(PathBuf::from("/sky")), 0.0);
    assert!(
        commands(&effects)
            .iter()
            .any(|c| matches!(c, ClientCommand::LoadRoutines { path } if path == &PathBuf::from("/sky")))
    );
}

#[test]
fn a_book_for_another_project_is_ignored() {
    let mut s = session(RunId::new());
    s.apply(Action::Server(ServerEvent::Routines { path: "/other".into(), book: RoutineBook::default() }), 0.0);
    assert_eq!(s.routines.book.routines.len(), 1);
    assert_eq!(s.unread_reports(), 1);
}

#[test]
fn opening_the_screen_asks_again_and_escape_steps_back() {
    let run = RunId::new();
    let mut s = session(run);
    let effects = routine(&mut s, RoutineAction::Open);
    assert!(s.routines.open);
    assert!(matches!(
        commands(&effects)[..],
        [ClientCommand::LoadRoutines { .. }, ClientCommand::ListPlugins, ClientCommand::ListChannels]
    ));

    routine(&mut s, RoutineAction::Select(run));
    routine(&mut s, RoutineAction::Back);
    assert_eq!(s.routines.selected, None);
    assert!(s.routines.open);
    routine(&mut s, RoutineAction::New);
    routine(&mut s, RoutineAction::Back);
    assert!(s.routines.form.is_none());
    routine(&mut s, RoutineAction::Back);
    assert!(!s.routines.open);
}

#[test]
fn opening_an_unread_report_marks_it_read() {
    let run = RunId::new();
    let mut s = session(run);
    let effects = routine(&mut s, RoutineAction::Select(run));
    assert_eq!(s.routines.selected, Some(run));
    assert_eq!(edits(&effects), vec![RoutineEdit::Read { run }]);
    routine(&mut s, RoutineAction::Select(run));
    assert_eq!(s.routines.selected, None);
}

#[test]
fn a_new_routine_is_written_and_saved_as_a_daily_draft_with_the_session_model() {
    let mut s = session(RunId::new());
    routine(&mut s, RoutineAction::New);
    type_in(&mut s, "Morning triage");
    routine(&mut s, RoutineAction::Submit);
    assert_eq!(s.routines.form.as_ref().map(|f| f.focus), Some(RoutineField::Brief));
    type_in(&mut s, "Look at new Sentry issues");
    let effects = routine(&mut s, RoutineAction::Submit);

    let [RoutineEdit::Create { draft }] = &edits(&effects)[..] else { panic!("expected a create") };
    assert_eq!(draft.name, "Morning triage");
    assert_eq!(draft.brief, "Look at new Sentry issues");
    assert_eq!(draft.schedule, Schedule::Daily { minute: 9 * 60, utc_offset: -3 * 3_600 });
    assert_eq!(draft.autonomy, Autonomy::Draft);
    assert_eq!(draft.model.as_deref(), s.models.get(s.conversation().model).map(|m| m.id.as_str()));
    assert!(s.routines.form.is_none());
}

#[test]
fn an_incomplete_form_is_not_saved() {
    let mut s = session(RunId::new());
    routine(&mut s, RoutineAction::New);
    type_in(&mut s, "Only a name");
    assert!(edits(&routine(&mut s, RoutineAction::Save)).is_empty());
    assert!(s.routines.form.is_some());
}

#[test]
fn weekly_is_the_local_weekday_it_was_written_on() {
    let mut s = session(RunId::new());
    routine(&mut s, RoutineAction::New);
    routine(&mut s, RoutineAction::Cadence(Cadence::Weekly));
    let schedule = s.routines.form.as_ref().map(|f| f.schedule);
    assert_eq!(schedule, Some(Schedule::Weekly { weekday: 6, minute: 9 * 60, utc_offset: -3 * 3_600 }));
}

#[test]
fn revising_keeps_the_schedule_and_sends_a_revise() {
    let mut s = session(RunId::new());
    routine(&mut s, RoutineAction::Revise(RoutineId(0)));
    routine(&mut s, RoutineAction::Autonomy(Autonomy::Bounded));
    let effects = routine(&mut s, RoutineAction::Save);
    let [RoutineEdit::Revise { id, draft }] = &edits(&effects)[..] else { panic!("expected a revise") };
    assert_eq!(*id, RoutineId(0));
    assert_eq!(draft.schedule, Schedule::Every { minutes: 60 });
    assert_eq!(draft.autonomy, Autonomy::Bounded);
}

#[test]
fn toggling_running_and_deleting_go_to_the_backend() {
    let mut s = session(RunId::new());
    let id = RoutineId(0);
    assert_eq!(edits(&routine(&mut s, RoutineAction::Toggle(id))), vec![RoutineEdit::Enable { id, enabled: false }]);
    assert_eq!(edits(&routine(&mut s, RoutineAction::RunNow(id))), vec![RoutineEdit::RunNow { id }]);
    assert_eq!(edits(&routine(&mut s, RoutineAction::Delete(id))), vec![RoutineEdit::Delete { id }]);
}

#[test]
fn continuing_a_report_opens_a_session_with_it_in_the_composer() {
    let run = RunId::new();
    let mut s = session(run);
    s.apply(Action::Edit(EditKey::Insert("busy".into())), 0.0);
    let before = s.active;
    routine(&mut s, RoutineAction::Open);
    routine(&mut s, RoutineAction::Continue(run));
    assert!(!s.routines.open);
    assert_ne!(s.active, before);
    let text = s.conversation().composer.text();
    assert!(text.starts_with("The routine \"Upkeep\" reported:"));
    assert!(text.contains("one is a regression"));
}

#[test]
fn opening_and_closing_the_screen_sounds_like_a_transition() {
    let mut s = session(RunId::new());
    let cue = |effects: Vec<Effect>| {
        effects.into_iter().find_map(|e| match e {
            Effect::Sound(cue) => Some(cue),
            _ => None,
        })
    };
    assert_eq!(cue(routine(&mut s, RoutineAction::Open)), Some(superfik_sound::Cue::TransitionUp));
    assert_eq!(cue(routine(&mut s, RoutineAction::Close)), Some(superfik_sound::Cue::TransitionDown));
}

fn account(plugin: &str) -> superfik_core::Connection {
    superfik_core::Connection {
        id: superfik_core::ConnectionId::new(),
        plugin: superfik_core::PluginId(plugin.into()),
        plugin_name: plugin.into(),
        label: String::new(),
        icon: String::new(),
    }
}

#[test]
fn opening_the_screen_asks_for_the_connected_accounts() {
    let mut s = session(RunId::new());
    let effects = routine(&mut s, RoutineAction::Open);
    assert!(commands(&effects).iter().any(|c| matches!(c, ClientCommand::ListPlugins)));
}

#[test]
fn a_template_fills_a_new_routine_with_its_plugins_accounts() {
    let mut s = session(RunId::new());
    let (sentry, gmail) = (account("sentry"), account("gmail"));
    s.plugins.connections = vec![sentry.clone(), gmail.clone()];
    routine(&mut s, RoutineAction::New);
    routine(&mut s, RoutineAction::Template(superfik_core::routine::Template::ProductionUpkeep));
    let form = s.routines.form.clone().unwrap_or_else(|| panic!("no form"));
    assert_eq!(form.name.text(), "Production upkeep");
    assert_eq!(form.autonomy, Autonomy::Bounded);
    assert_eq!(form.connections, vec![sentry.clone()]);

    routine(&mut s, RoutineAction::Account(gmail.id));
    routine(&mut s, RoutineAction::Account(sentry.id));
    let effects = routine(&mut s, RoutineAction::Save);
    let [RoutineEdit::Create { draft }] = &edits(&effects)[..] else { panic!("expected a create") };
    assert_eq!(draft.connections, vec![gmail]);
}

#[test]
fn a_template_never_overwrites_a_routine_being_revised() {
    let mut s = session(RunId::new());
    routine(&mut s, RoutineAction::Revise(RoutineId(0)));
    routine(&mut s, RoutineAction::Template(superfik_core::routine::Template::RevenueDigest));
    assert_eq!(s.routines.form.as_ref().map(|f| f.name.text().to_owned()), Some("Upkeep".into()));
}
