//! Side calls through `AgentEngine::start`: a title before any session, a
//! suggestion on a kept one, and the output style picked in Settings.

use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use superfik_core::{ChoreAnswer, EngineEvent, ModelOption, PromptRequest, RunId, SessionCommand};
use superfik_engine::AgentEngine;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::model::{ModelApi, ModelError};
use superfik_native::sessions::{InMemorySessions, Sessions};
use superfik_native::transcript::{Block, Message};
use superfik_native::{ApiSource, NativeEngine};

struct Source(Mutex<Vec<Arc<ScriptedModel>>>);

impl ApiSource for Source {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(self.0.lock().expect("models").remove(0))
    }
}

fn finish(engine: &NativeEngine, request: &PromptRequest) -> Vec<EngineEvent> {
    let (send, receive): (_, Receiver<EngineEvent>) = channel();
    let send = Mutex::new(send);
    let sink = Arc::new(move |_run: RunId, event: EngineEvent| {
        let _ = send.lock().expect("send").send(event);
    });
    engine.start(RunId::new(), request, sink).expect("start");
    let mut seen = Vec::new();
    while let Ok(event) = receive.recv_timeout(Duration::from_secs(10)) {
        let terminal = event.is_terminal();
        seen.push(event);
        if terminal {
            break;
        }
    }
    seen
}

fn says(text: &str) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text(text.into())])]))
}

fn model() -> ModelOption {
    ModelOption::new("claude-opus-5-5", "Opus 5.5")
}

#[test]
fn a_title_needs_no_session_and_keeps_none() {
    let dir = tempfile::tempdir().expect("dir");
    let titler = says("{\"title\": \"Login page\"}");
    let sessions = Arc::new(InMemorySessions::default());
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(&titler)]))), sessions);
    let mut request = PromptRequest::plain(dir.path().into(), "fix the login page please", model());
    request.command = Some(SessionCommand::Title);
    let events = finish(&engine, &request);
    assert!(events.contains(&EngineEvent::Chore(ChoreAnswer::Title("Login page".into()))), "{events:?}");
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })));
    assert!(!events.iter().any(|e| matches!(e, EngineEvent::SessionStarted(_))), "a title starts no session");
    let asked = &titler.asked()[0];
    assert!(asked.tools.is_empty());
    assert!(asked.messages[0].text().contains("<session>\nfix the login page please\n</session>"));
}

#[test]
fn a_suggestion_reads_the_kept_session() {
    let dir = tempfile::tempdir().expect("dir");
    let sessions = Arc::new(InMemorySessions::default());
    let mut request = PromptRequest::plain(dir.path().into(), "", model());
    let id = superfik_core::SessionId("kept".into());
    let history = vec![Message::user("fix the bug"), Message::assistant(vec![Block::Text("Fixed it.".into())])];
    sessions.save_for(&request, &id, &history).expect("save");
    let guesser = says("run the tests");
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(&guesser)]))), sessions);
    request.resume = Some(id);
    request.command = Some(SessionCommand::Suggest);
    let events = finish(&engine, &request);
    assert!(events.contains(&EngineEvent::Chore(ChoreAnswer::Suggestion("run the tests".into()))), "{events:?}");
    let asked = &guesser.asked()[0];
    assert_eq!(&asked.messages[..2], &history[..]);
    assert!(asked.messages[2].text().starts_with("[SUGGESTION MODE"));
}

#[test]
fn the_style_picked_in_settings_reaches_the_prompt() {
    let dir = tempfile::tempdir().expect("dir");
    let model_api = says("Done.");
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&model_api)]))),
        Arc::new(InMemorySessions::default()),
    );
    let mut request = PromptRequest::plain(dir.path().into(), "hi", model());
    request.output_style = Some("concise".into());
    finish(&engine, &request);
    let asked = &model_api.asked()[0];
    assert!(asked.system.contains("# Concise style"), "the style's text");
    assert!(asked.system.contains("Be concise: lead with the result"), "and its turn reminder");
}

#[test]
fn a_side_question_on_a_session_not_kept_yet_is_answered_not_reported_lost() {
    let dir = tempfile::tempdir().expect("dir");
    let answerer = says("Nothing has been said yet.");
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&answerer)]))),
        Arc::new(InMemorySessions::default()),
    );
    let mut request = PromptRequest::plain(dir.path().into(), "what is it doing?", model());
    request.resume = Some(superfik_core::SessionId("first-run".into()));
    request.command = Some(SessionCommand::Side);
    let events = finish(&engine, &request);
    assert!(!events.iter().any(|e| matches!(e, EngineEvent::Diagnosed(_))), "{events:?}");
    assert!(events.contains(&EngineEvent::Chore(ChoreAnswer::Side("Nothing has been said yet.".into()))));
}
