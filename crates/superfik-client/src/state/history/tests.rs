use std::path::PathBuf;

use superfik_core::{Activity, EngineEvent, RunId};
use superfik_protocol::ServerEvent;

use super::*;
use crate::state::tests::commands;
use crate::state::{Action, EditKey};

fn session() -> Session {
    Session::new(Some(PathBuf::from("/repo")))
}

fn saved(effects: &[Effect]) -> Vec<SessionRecord> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::KeepSession { path, session, .. } if path == Path::new("/repo") => Some((**session).clone()),
            _ => None,
        })
        .collect()
}

fn submit(session: &mut Session, text: &str) -> (RunId, Vec<Effect>) {
    session.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
    let effects = session.apply(Action::Submit, 1.0);
    let run = commands(&effects)
        .into_iter()
        .find_map(|c| match c {
            ClientCommand::SubmitPrompt { run, .. } => Some(*run),
            _ => None,
        })
        .unwrap_or_else(RunId::new);
    (run, effects)
}

fn event(run: RunId, event: EngineEvent) -> Action {
    Action::Server(ServerEvent::Run { run, event })
}

#[test]
fn a_session_is_kept_when_it_starts_and_when_its_reply_ends_not_while_text_streams() {
    let mut s = session();
    let (run, effects) = submit(&mut s, "Build a sky");
    let first = saved(&effects);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].title.as_deref(), Some("Build a sky"));
    s.apply(event(run, EngineEvent::TextDelta("Su".into())), 1.4);
    assert!(saved(&s.apply(event(run, EngineEvent::TextDelta("re".into())), 1.5)).is_empty());
    let done = saved(&s.apply(event(run, EngineEvent::Completed { duration_ms: 3, cost_usd: None }), 2.0));
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].turns[0].response.text, "Sure");
    assert_eq!(done[0].turns[0].response.activity, Activity::Done);
}

#[test]
fn renaming_archiving_and_effort_are_kept() {
    let mut s = session();
    let (run, _) = submit(&mut s, "Build a sky");
    s.apply(event(run, EngineEvent::Completed { duration_ms: 3, cost_usd: None }), 2.0);
    let id = s.active;
    s.apply(Action::BeginRename(id), 3.0);
    s.apply(Action::Edit(EditKey::Insert("Night".into())), 3.0);
    assert_eq!(saved(&s.apply(Action::CommitRename, 3.0))[0].title.as_deref(), Some("Night"));
    assert_eq!(saved(&s.apply(Action::SetEffort(superfik_core::Effort::Max), 3.0)).len(), 1);
    assert!(saved(&s.apply(Action::ArchiveConversation(id), 3.0))[0].archived);
    assert!(!saved(&s.apply(Action::UnarchiveConversation(id), 4.0))[0].archived, "restoring is kept too");
}

#[test]
fn an_untouched_session_is_never_kept() {
    let mut s = session();
    assert!(saved(&s.apply(Action::NewConversation, 0.0)).is_empty());
    assert!(saved(&s.apply(Action::SetEffort(superfik_core::Effort::Max), 0.0)).is_empty());
}

#[test]
fn opening_a_workspace_asks_for_its_sessions() {
    let mut s = session();
    let effects = s.apply(Action::OpenWorkspace("/other".into()), 0.0);
    assert!(commands(&effects).contains(&&ClientCommand::LoadSessions { path: "/other".into(), project: None }));
}

fn kept_elsewhere() -> SessionRecord {
    let mut s = session();
    let (run, _) = submit(&mut s, "Build a sky");
    s.apply(event(run, EngineEvent::SessionStarted(superfik_core::SessionId("e-1".into()))), 1.2);
    s.apply(event(run, EngineEvent::TextDelta("Half".into())), 1.5);
    s.conversation().record(&s.models).expect("started")
}

#[test]
fn kept_sessions_join_the_timeline_once_without_being_saved_back() {
    let mut s = session();
    let record = kept_elsewhere();
    let arrived = Action::Server(ServerEvent::Sessions { path: "/repo".into(), sessions: vec![record.clone()] });
    assert!(saved(&s.apply(arrived.clone(), 0.0)).is_empty());
    s.apply(arrived, 0.0);
    assert_eq!(s.conversations.len(), 2);
    let back = s.conversations.iter().find(|c| c.id == record.id).expect("restored");
    assert_eq!(back.title.as_deref(), Some("Build a sky"));
    assert_eq!(back.engine_session, Some(superfik_core::SessionId("e-1".into())));
    assert_eq!(back.turns[0].response.activity, Activity::Cancelled, "the app closed mid-reply");
    assert!(!back.is_running());
    assert_ne!(s.active, record.id, "the fresh session stays on the canvas");
}

#[test]
fn sessions_of_another_workspace_are_ignored() {
    let mut s = session();
    let arrived = ServerEvent::Sessions { path: "/elsewhere".into(), sessions: vec![kept_elsewhere()] };
    s.apply(Action::Server(arrived), 0.0);
    assert_eq!(s.conversations.len(), 1);
}

#[test]
fn a_kept_session_keeps_its_own_model() {
    let mut s = session();
    let mut record = kept_elsewhere();
    record.model = s.models[2].id.clone();
    s.apply(Action::Server(ServerEvent::Sessions { path: "/repo".into(), sessions: vec![record.clone()] }), 0.0);
    s.apply(Action::Server(ServerEvent::Models(s.models.iter().rev().cloned().collect())), 0.0);
    let back = s.conversations.iter().find(|c| c.id == record.id).expect("restored");
    assert_eq!(s.models[back.model].id, record.model);
    let unknown = SessionRecord { model: "gone".into(), ..record };
    assert_eq!(Conversation::from_record(unknown, &s.models).model, 0);
}
