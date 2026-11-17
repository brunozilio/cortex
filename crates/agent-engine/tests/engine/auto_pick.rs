use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{Effort, EngineEvent, PromptRequest};

use crate::{Source, finish, model, start};

fn engine(model: &Arc<ScriptedModel>) -> Engine {
    Engine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(model)]))), Arc::new(InMemorySessions::default()))
}

fn auto(prompt: &str) -> (tempfile::TempDir, PromptRequest) {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(&root).expect("settings dir");
    std::fs::write(root.join("settings.json"), r#"{"smallModel":"claude-haiku-5-5"}"#).expect("settings");
    let mut request = PromptRequest::plain(dir.path().into(), prompt, model());
    request.session.auto_pick = true;
    (dir, request)
}

#[test]
fn the_small_model_picks_the_model_and_effort_for_the_request() {
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text(r#"{"tier":"small","effort":"low"}"#.into())]),
        ScriptedModel::reply(vec![Block::Text("Renamed.".into())]),
    ]));
    let (_dir, request) = auto("rename the variable x to y");
    let events = finish(&start(&engine(&scripted), &request));

    let asked = scripted.asked();
    assert!(asked[0].tools.is_empty(), "the pick only reads the request");
    assert_eq!(asked[1].model, "claude-haiku-5-5", "the run went to the small model");
    assert_eq!(asked[1].effort, Effort::Low);
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Picked { model, effort: Effort::Low } if *model == asked[1].model
    )));
}

#[test]
fn a_bare_go_ahead_costs_no_pick() {
    let scripted = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Going on.".into())])]));
    let (_dir, request) = auto("continue");
    finish(&start(&engine(&scripted), &request));

    let asked = scripted.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!((asked[0].model.as_str(), asked[0].effort), (request.session.model.id.as_str(), Effort::Low));
}

#[test]
fn an_unreadable_pick_leaves_the_session_s_model_at_medium() {
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text("hmm".into())]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]));
    let (_dir, request) = auto("explain the build");
    finish(&start(&engine(&scripted), &request));

    let asked = scripted.asked();
    assert_eq!((asked[1].model.as_str(), asked[1].effort), (request.session.model.id.as_str(), Effort::Medium));
}
