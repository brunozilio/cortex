use agent_engine::agent::Ending;
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
    rig.workspace.hooks = Hooks::parse(&answering("ModelRequest", "main", "keyed:model-b")).expect("hooks");
    let model = ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Done.".into())])]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    assert_eq!(model.asked()[0].model, "keyed:model-b");
    let input = std::fs::read_to_string(rig.dir.path().join("ModelRequest.json")).expect("input");
    let input: serde_json::Value = serde_json::from_str(&input).expect("json");
    assert_eq!((input["model"].as_str(), input["kind"].as_str()), (Some("m"), Some("main")));
    assert_eq!(input["mode"], "bypass");
}

#[cfg(unix)]
#[test]
fn a_model_error_hook_names_the_model_to_try_next() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.hooks = Hooks::parse(&answering("ModelError", "", "keyed-too:auto")).expect("hooks");
    let model = ScriptedModel::new([
        Err(ModelError::Overloaded("busy".into())),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    assert_eq!(model.asked()[1].model, "keyed-too:auto");
    let input = std::fs::read_to_string(rig.dir.path().join("ModelError.json")).expect("input");
    assert!(input.contains("busy"), "{input}");
}

#[test]
fn a_model_error_hook_intercepts_every_failure_and_chains_models() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    let folder = rig.dir.path().join("hooks");
    std::fs::create_dir_all(&folder).expect("hooks");
    std::fs::write(
        folder.join("chain.js"),
        r#"export const event = "ModelError";
const chain = ["first:a", "second:b"];
export default (input) => ({ hookSpecificOutput: { model: chain[input.attempt - 1] } });"#,
    )
    .expect("hook");
    rig.workspace.hooks = Hooks::from_scripts(&folder);
    let model = ScriptedModel::new([
        Err(ModelError::Unauthorized("no key".into())),
        Err(ModelError::Network("down".into())),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    let models: Vec<String> = model.asked().into_iter().map(|asked| asked.model).collect();
    assert_eq!(models, ["m", "first:a", "second:b"]);
}

#[cfg(unix)]
#[test]
fn a_model_error_hook_sees_the_failure_the_models_tried_and_the_fallbacks() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.fallbacks = vec!["backup".into()];
    let command = "cat > ModelError.json";
    rig.workspace.hooks =
        Hooks::parse(&json!({"ModelError": [{"hooks": [{"type": "command", "command": command}]}]})).expect("hooks");
    let model = ScriptedModel::new([
        Err(ModelError::Overloaded("busy".into())),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("hi")]).expect("run");
    let input = std::fs::read_to_string(rig.dir.path().join("ModelError.json")).expect("input");
    let input: serde_json::Value = serde_json::from_str(&input).expect("json");
    assert_eq!(input["errorKind"], "overloaded");
    assert_eq!(input["attempt"], 1);
    assert_eq!(input["tried"], json!(["m"]));
    assert_eq!(input["fallbacks"], json!(["backup"]));
    assert_eq!(model.asked()[1].model, "backup", "a silent hook leaves the fallback chain to answer");
}

#[cfg(unix)]
#[test]
fn a_model_error_hook_may_stop_the_fallback() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.fallbacks = vec!["backup".into()];
    let command = r#"printf '%s' '{"decision":"block","reason":"stay on m"}'"#;
    rig.workspace.hooks =
        Hooks::parse(&json!({"ModelError": [{"hooks": [{"type": "command", "command": command}]}]})).expect("hooks");
    let model = ScriptedModel::new([Err(ModelError::Overloaded("busy".into()))]);
    let ending = rig.run(&model, &mut vec![Message::user("hi")]);
    assert_eq!(ending, Err(Ending::Failed(ModelError::Overloaded("busy".into()))));
    assert_eq!(model.asked().len(), 1);
}

#[test]
fn a_model_hook_cannot_ask_a_model() {
    let prompt = json!({"ModelRequest": [{"hooks": [{"type": "prompt", "prompt": "pick one"}]}]});
    assert!(Hooks::parse(&prompt).is_err());
}
