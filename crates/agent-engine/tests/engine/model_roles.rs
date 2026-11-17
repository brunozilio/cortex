use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::model_roles::ModelRole;
use domain::{EngineEvent, PermissionMode, PromptRequest};

use crate::{Source, finish, model, start};

const GROK: &str = "grok-subscription:grok-4.6";

fn engine(model: &Arc<ScriptedModel>) -> Engine {
    Engine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(model)]))), Arc::new(InMemorySessions::default()))
}

fn request(dir: &tempfile::TempDir, role: ModelRole, chosen: &str) -> PromptRequest {
    let mut request = PromptRequest::plain(dir.path().into(), "Look around", model());
    request.policy.permission = PermissionMode::Bypass;
    request.session.roles.set(role, Some(chosen.into()));
    request
}

#[cfg(feature = "subscription-providers")]
fn agent_call(kind: &str) -> Block {
    Block::ToolUse {
        id: "agent".into(),
        name: "Agent".into(),
        input: serde_json::json!({"subagent_type": kind, "description": "look", "prompt": "Find the entry point."}),
    }
}

#[test]
fn plan_mode_turns_go_to_the_plan_model_even_on_another_provider() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("A plan.".into())])]));
    let mut request = request(&dir, ModelRole::Plan, GROK);
    request.policy.permission = PermissionMode::Plan;
    let events = finish(&start(&engine(&scripted), &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert_eq!(scripted.asked()[0].model, GROK);
}

#[test]
fn outside_plan_mode_the_session_model_does_the_work() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done.".into())])]));
    finish(&start(&engine(&scripted), &request(&dir, ModelRole::Plan, GROK)));
    assert_eq!(scripted.asked()[0].model, model().id);
}

#[cfg(feature = "subscription-providers")]
#[test]
fn the_explore_agent_runs_on_the_explore_model() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![agent_call("Explore")]),
        ScriptedModel::reply(vec![Block::Text("It is main.rs.".into())]),
        ScriptedModel::reply(vec![Block::Text("Found it.".into())]),
    ]));
    finish(&start(&engine(&scripted), &request(&dir, ModelRole::Explore, GROK)));
    let models: Vec<String> = scripted.asked().into_iter().map(|asked| asked.model).collect();
    assert_eq!(models, [model().id.as_str(), GROK, model().id.as_str()]);
}

#[cfg(feature = "subscription-providers")]
#[test]
fn the_plan_agent_runs_on_the_plan_model() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![agent_call("Plan")]),
        ScriptedModel::reply(vec![Block::Text("Step one.".into())]),
        ScriptedModel::reply(vec![Block::Text("Planned.".into())]),
    ]));
    finish(&start(&engine(&scripted), &request(&dir, ModelRole::Plan, GROK)));
    assert_eq!(scripted.asked()[1].model, GROK);
}

#[test]
fn the_small_role_wins_over_the_small_model_setting() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(&root).expect("settings dir");
    std::fs::write(root.join("settings.json"), r#"{"smallModel":"claude-haiku-5-5"}"#).expect("settings");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::Text(r#"{"tier":"small","effort":"low"}"#.into())]),
        ScriptedModel::reply(vec![Block::Text("Renamed.".into())]),
    ]));
    let mut request = request(&dir, ModelRole::Small, GROK);
    request.prompt.text = "rename the variable x to y".into();
    request.session.auto_pick = true;
    finish(&start(&engine(&scripted), &request));
    assert_eq!(scripted.asked()[1].model, GROK);
}

#[test]
fn the_session_s_fallbacks_take_over_when_the_model_is_busy() {
    let dir = tempfile::tempdir().expect("dir");
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(&root).expect("settings dir");
    std::fs::write(root.join("settings.json"), r#"{"fallbackModel":"from-settings"}"#).expect("settings");
    let scripted = Arc::new(ScriptedModel::new([
        Err(agent_engine::model::ModelError::Overloaded("429".into())),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]));
    let mut request = PromptRequest::plain(dir.path().into(), "go", model());
    request.policy.permission = PermissionMode::Bypass;
    request.session.fallbacks = vec![GROK.into()];
    finish(&start(&engine(&scripted), &request));
    assert_eq!(scripted.asked()[1].model, GROK, "the session's chain wins over the settings file");
}
