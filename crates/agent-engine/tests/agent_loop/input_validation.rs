use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_engine::model::ToolSpec;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::{Declared, Effect, Outcome, Tool, Workspace};
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::{Value, json};

use crate::{Rig, tool_use};

#[derive(Default)]
struct Counted {
    judged: AtomicUsize,
    ran: AtomicUsize,
}

struct Strict(Arc<Counted>);

impl Tool for Strict {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "Strict".into(),
            description: "Takes a path and a count.".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "count": { "type": "integer", "minimum": 1 },
                    "mode": { "type": "string", "enum": ["fast", "slow"] }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        self.0.judged.fetch_add(1, Ordering::SeqCst);
        vec![Effect::Declared(Declared::Executes)]
    }

    fn summary(&self, input: &Value) -> String {
        input["path"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        self.0.ran.fetch_add(1, Ordering::SeqCst);
        Outcome::ok("ran")
    }
}

fn results(history: &[Message]) -> Vec<(String, bool)> {
    history
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            Block::ToolResult { content, is_error, .. } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

fn refused(input: Value) -> (String, Arc<Counted>, Vec<EngineEvent>) {
    let rig = Rig::new(PermissionMode::Ask);
    let counted = Arc::new(Counted::default());
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("s", "Strict", input)]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("go")];
    rig.run_with(&model, &mut history, vec![Box::new(Strict(Arc::clone(&counted)))]).expect("run");
    let (content, is_error) = results(&history).pop().expect("a tool result");
    assert!(is_error, "{content}");
    (content, counted, rig.events())
}

fn never_judged_nor_run(counted: &Counted, events: &[EngineEvent]) {
    assert_eq!(counted.judged.load(Ordering::SeqCst), 0, "effects are not judged for an invalid input");
    assert_eq!(counted.ran.load(Ordering::SeqCst), 0, "the tool never runs");
    assert!(!events.iter().any(|event| matches!(event, EngineEvent::Asked(_))), "nobody is asked");
}

#[test]
fn a_missing_required_field_is_refused_naming_it_before_anything_is_judged_or_run() {
    let (content, counted, events) = refused(json!({ "count": 2 }));
    assert!(content.contains("Strict") && content.contains("path"), "{content}");
    never_judged_nor_run(&counted, &events);
}

#[test]
fn a_field_of_the_wrong_type_is_refused_naming_it() {
    let (content, counted, events) = refused(json!({ "path": "a", "count": "two" }));
    assert!(content.contains("count"), "{content}");
    never_judged_nor_run(&counted, &events);
}

#[test]
fn a_field_the_schema_does_not_allow_is_refused_naming_it() {
    let (content, counted, events) = refused(json!({ "path": "a", "sneaky": true }));
    assert!(content.contains("sneaky"), "{content}");
    never_judged_nor_run(&counted, &events);
}

#[test]
fn a_value_outside_its_enum_or_range_is_refused() {
    let (content, counted, events) = refused(json!({ "path": "a", "mode": "reckless" }));
    assert!(content.contains("mode"), "{content}");
    never_judged_nor_run(&counted, &events);
    let (content, ..) = refused(json!({ "path": "a", "count": 0 }));
    assert!(content.contains("count"), "{content}");
}

#[test]
fn a_built_in_tool_is_checked_against_its_declared_schema() {
    let rig = Rig::new(PermissionMode::Ask);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": "echo hi > out.txt", "as_root": true }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("go")];
    rig.run(&model, &mut history).expect("run");
    let (content, is_error) = results(&history).pop().expect("a tool result");
    assert!(is_error && content.contains("as_root"), "{content}");
    assert!(!rig.dir.path().join("out.txt").exists());
    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_))));
}

#[test]
fn a_valid_input_still_runs() {
    let rig = Rig::new(PermissionMode::Bypass);
    let counted = Arc::new(Counted::default());
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("s", "Strict", json!({ "path": "a", "count": 2, "mode": "fast" }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let mut history = vec![Message::user("go")];
    rig.run_with(&model, &mut history, vec![Box::new(Strict(Arc::clone(&counted)))]).expect("run");
    assert_eq!(counted.ran.load(Ordering::SeqCst), 1, "{:?}", results(&history));
}
