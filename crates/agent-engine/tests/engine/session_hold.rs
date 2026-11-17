use std::sync::{Arc, Mutex};

use crossbeam_channel::unbounded;
use std::time::Duration;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::{FileSessions, Sessions};
use agent_engine::transcript::Block;
use agent_engine::{AgentEngine, Engine};
use domain::{EngineEvent, PromptRequest, RunId, SessionId};

use crate::{Source, finish, model, start};

const ROUNDS: usize = 20;

fn answering(text: &str) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(text.into())])]))
}

struct Ended {
    terminal: EngineEvent,
    free_at_the_terminal_event: Result<(), String>,
}

fn run_to_its_end(
    engine: &Engine,
    sessions: &Arc<FileSessions>,
    request: &PromptRequest,
    session: &SessionId,
    cancel_at_once: bool,
) -> Ended {
    let (send, ended) = unbounded();
    let send = Mutex::new(send);
    let (store, asked, held) = (Arc::clone(sessions), request.clone(), session.clone());
    let sink = Arc::new(move |_run: RunId, event: EngineEvent| {
        if event.is_terminal() {
            let free_at_the_terminal_event = store.hold(&asked.session.cwd, &held).map(drop);
            let _ = send.lock().expect("send").send(Ended { terminal: event, free_at_the_terminal_event });
        }
    });
    let mut handle = engine.start(RunId::new(), request, sink).expect("start");
    if cancel_at_once {
        handle.cancel();
    }
    ended.recv_timeout(Duration::from_secs(10)).expect("the run ends")
}

fn kept_session(dir: &std::path::Path, later: Vec<Arc<ScriptedModel>>) -> (Engine, Arc<FileSessions>, SessionId) {
    let sessions = Arc::new(FileSessions::new(dir.join("native")));
    let mut models = vec![answering("One.")];
    models.extend(later);
    let engine = Engine::new(Arc::new(Source(Mutex::new(models))), Arc::clone(&sessions) as _);
    let events = finish(&start(&engine, &PromptRequest::plain(dir.into(), "one", model())));
    let Some(EngineEvent::SessionStarted(session)) = events.first().cloned() else { panic!("{events:?}") };
    (engine, sessions, session)
}

fn resuming(dir: &std::path::Path, session: &SessionId) -> PromptRequest {
    let mut again = PromptRequest::plain(dir.into(), "again", model());
    again.session.resume = Some(session.clone());
    again
}

#[test]
fn a_session_can_be_resumed_the_moment_its_run_completes() {
    let dir = tempfile::tempdir().expect("dir");
    let (engine, sessions, session) = kept_session(dir.path(), (0..ROUNDS).map(|_| answering("More.")).collect());
    let again = resuming(dir.path(), &session);
    for round in 0..ROUNDS {
        let ended = run_to_its_end(&engine, &sessions, &again, &session, false);
        assert!(matches!(ended.terminal, EngineEvent::Completed { .. }), "round {round}: {:?}", ended.terminal);
        assert_eq!(ended.free_at_the_terminal_event, Ok(()), "round {round}");
    }
    let kept = sessions.load_for(&again.session.cwd, &session).map(|history| history.len());
    assert_eq!(kept, Some(2 + 2 * ROUNDS), "every run saved its turn before it ended");
}

#[test]
fn a_session_can_be_resumed_the_moment_its_run_fails() {
    let dir = tempfile::tempdir().expect("dir");
    let (engine, sessions, session) = kept_session(dir.path(), Vec::new());
    let again = resuming(dir.path(), &session);
    for round in 0..ROUNDS {
        let ended = run_to_its_end(&engine, &sessions, &again, &session, false);
        assert!(matches!(ended.terminal, EngineEvent::Failed(_)), "round {round}: {:?}", ended.terminal);
        assert_eq!(ended.free_at_the_terminal_event, Ok(()), "round {round}");
    }
}

#[test]
fn a_session_can_be_resumed_the_moment_its_run_is_cancelled() {
    let dir = tempfile::tempdir().expect("dir");
    let (engine, sessions, session) = kept_session(dir.path(), (0..ROUNDS).map(|_| answering("More.")).collect());
    let again = resuming(dir.path(), &session);
    for round in 0..ROUNDS {
        let ended = run_to_its_end(&engine, &sessions, &again, &session, true);
        assert!(ended.terminal.is_terminal());
        assert_eq!(ended.free_at_the_terminal_event, Ok(()), "round {round}: {:?}", ended.terminal);
    }
}
