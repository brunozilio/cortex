use agent_engine::hooks::Hooks;
use agent_engine::model::ModelError;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::PermissionMode;
use serde_json::json;

use crate::Rig;

fn answering(event: &str, matcher: &str, model: &str) -> serde_json::Value {
    let output = format!(r#"{{"hookSpecificOutput":{{"model":"{model}"}}}}"#);
    let command = format!("cat > {event}.json; printf '%s' '{output}'");
    json!({event: [{"matcher": matcher, "hooks": [{"type": "command", "command": command}]}]})
}

#[cfg(unix)]
#[test]
fn a_model_request_hook_picks_the_model_of_each_turn() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = Hooks::parse(&answering("ModelRequest", "main", "openai:gpt-6.1-sol")).expect("hooks");
    let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done.".into())])]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    assert_eq!(model.asked()[0].model, "openai:gpt-6.1-sol");
    let input = std::fs::read_to_string(rig.dir.path().join("ModelRequest.json")).expect("input");
    let input: serde_json::Value = serde_json::from_str(&input).expect("json");
    assert_eq!((input["model"].as_str(), input["kind"].as_str()), (Some("m"), Some("main")));
    assert_eq!(input["mode"], "bypass");
}

#[cfg(unix)]
#[test]
fn a_model_error_hook_names_the_model_to_try_next() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = Hooks::parse(&answering("ModelError", "", "kiro-subscription:auto")).expect("hooks");
    let model = ScriptedModel::new([
        Err(ModelError::Overloaded("busy".into())),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    assert_eq!(model.asked()[1].model, "kiro-subscription:auto");
    let input = std::fs::read_to_string(rig.dir.path().join("ModelError.json")).expect("input");
    assert!(input.contains("busy"), "{input}");
}

#[test]
fn a_model_hook_cannot_ask_a_model() {
    let prompt = json!({"ModelRequest": [{"hooks": [{"type": "prompt", "prompt": "pick one"}]}]});
    assert!(Hooks::parse(&prompt).is_err());
}
