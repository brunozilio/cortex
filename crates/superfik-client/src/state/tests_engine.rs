//! Reducer rules around the engine: conversation continuity, problems
//! surfaced as notices, and custom agents.

use std::path::PathBuf;

use superfik_core::{
    CustomAgent, EngineEvent, EngineKind, EngineProblem, ModelOption, PromptRequest, RunId, SessionId,
};
use superfik_protocol::{ClientCommand, ServerEvent};

use super::*;

fn session() -> Session {
    let mut s = Session::new(Some(PathBuf::from("/repo")));
    s.models.push(ModelOption::new(EngineKind::Codex, "gpt", "GPT"));
    s
}

fn submit(s: &mut Session, text: &str) -> (RunId, PromptRequest) {
    s.apply(Action::Edit(EditKey::Insert(text.into())), 0.0);
    let effects = s.apply(Action::Submit, 1.0);
    match super::tests::commands(&effects).as_slice() {
        [ClientCommand::SubmitPrompt { run, request }]
        | [ClientCommand::CancelRun { .. }, ClientCommand::SubmitPrompt { run, request }] => (*run, request.clone()),
        other => panic!("unexpected {other:?}"),
    }
}

fn event(s: &mut Session, run: RunId, event: EngineEvent) {
    s.apply(Action::Server(ServerEvent::Run { run, event }), 2.0);
}

fn finish(s: &mut Session, run: RunId) {
    event(s, run, EngineEvent::Completed { duration_ms: 10, cost_usd: None });
}

#[test]
fn follow_ups_resume_the_engine_session() {
    let mut s = session();
    let (run, first) = submit(&mut s, "hello");
    assert_eq!(first.resume, None);
    event(&mut s, run, EngineEvent::SessionStarted(SessionId("abc".into())));
    finish(&mut s, run);
    let (_, second) = submit(&mut s, "and then?");
    assert_eq!(second.resume, Some(SessionId("abc".into())));
}

#[test]
fn model_and_effort_changes_keep_the_conversation() {
    let mut s = session();
    s.conversation_mut().engine_session = Some(SessionId("abc".into()));
    s.apply(Action::SelectModel(3), 0.0);
    s.apply(Action::SetEffort(superfik_core::Effort::High), 0.0);
    let (_, request) = submit(&mut s, "faster now");
    assert_eq!(request.resume, Some(SessionId("abc".into())));
    assert_eq!(request.model.label, "Haiku 5.5");
    assert_eq!(request.effort, superfik_core::Effort::High);
}

#[test]
fn switching_engine_or_workspace_starts_a_new_conversation() {
    let mut s = session();
    s.conversation_mut().engine_session = Some(SessionId("abc".into()));
    s.apply(Action::SelectModel(4), 0.0);
    assert_eq!(s.model().engine, EngineKind::Codex);
    assert_eq!(s.conversation().engine_session, None);

    let mut s = session();
    s.conversation_mut().engine_session = Some(SessionId("abc".into()));
    let effects = s.apply(Action::OpenWorkspace("/other".into()), 0.0);
    assert_eq!(s.conversation().engine_session, None);
    assert!(effects.iter().any(
        |e| matches!(e, Effect::Command(c) if matches!(c.as_ref(), ClientCommand::ListAgents { path } if path == &PathBuf::from("/other")))
    ));
}

#[test]
fn lost_session_is_dropped_with_a_notice() {
    let mut s = session();
    s.conversation_mut().engine_session = Some(SessionId("gone".into()));
    let (run, _) = submit(&mut s, "hi");
    event(&mut s, run, EngineEvent::Diagnosed(EngineProblem::SessionNotFound));
    event(&mut s, run, EngineEvent::Failed("No conversation found with session ID: gone".into()));
    assert_eq!(s.conversation().engine_session, None);
    assert!(s.notice_text().as_deref().is_some_and(|n| n.contains("starts a new one")));
}

#[test]
fn not_logged_in_shows_a_clear_notice_not_the_raw_error() {
    let mut s = session();
    let (run, _) = submit(&mut s, "hi");
    event(&mut s, run, EngineEvent::Diagnosed(EngineProblem::NotLoggedIn));
    event(&mut s, run, EngineEvent::Failed("Not logged in · Please run /login".into()));
    assert_eq!(s.notice, Some(super::Notice::Engine(EngineProblem::NotLoggedIn, EngineKind::Native)));
    assert!(!s.is_running());
}

#[test]
fn plain_failures_still_surface() {
    let mut s = session();
    let (run, _) = submit(&mut s, "hi");
    event(&mut s, run, EngineEvent::Failed("Overloaded".into()));
    assert_eq!(s.notice_text().as_deref(), Some("Overloaded"));
}

#[test]
fn spawned_custom_agents_show_their_description() {
    let mut s = session();
    let reviewer = CustomAgent { name: "code-reviewer".into(), description: "Reviews diffs".into() };
    s.apply(Action::Server(ServerEvent::Agents(vec![reviewer])), 0.0);
    let (run, request) = submit(&mut s, "review it");
    assert!(!request.compose().contains("sub-agents"), "the main agent decides on sub-agents");
    let spawned = EngineEvent::AgentSpawned {
        index: 0,
        kind: "code-reviewer".into(),
        summary: String::new(),
        prompt: String::new(),
    };
    event(&mut s, run, spawned);
    let agent = &s.conversation().agents[0];
    assert_eq!((agent.label(), agent.summary.as_str()), ("code-reviewer", "Reviews diffs"));
}

#[test]
fn long_model_catalogs_scroll_within_the_menu_and_reset_when_reopened() {
    let mut session = Session::default();
    session.apply(Action::OpenPopover(super::PopoverKind::Model), 0.0);
    session.apply(Action::ScrollModels { by: 5000.0, max: 700.0 }, 0.0);
    assert_eq!(session.model_scroll, 700.0);
    session.apply(Action::ScrollModels { by: -5000.0, max: 700.0 }, 0.0);
    assert_eq!(session.model_scroll, 0.0);
    session.apply(Action::ScrollModels { by: 140.0, max: 700.0 }, 0.0);
    session.apply(Action::OpenPopover(super::PopoverKind::Model), 0.0);
    session.apply(Action::OpenPopover(super::PopoverKind::Model), 0.0);
    assert_eq!(session.model_scroll, 0.0);
}
