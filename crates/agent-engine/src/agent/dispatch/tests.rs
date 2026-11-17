use std::sync::{Arc, Mutex};

use domain::EngineEvent;
use platform::sync::Locked;
use serde_json::{Value, json};

use crate::agent::Agent;
use crate::agent::run::ThisRun;
use crate::control::Control;
use crate::hooks::Hooks;
use crate::model::scripted::ScriptedModel;
use crate::model::{ToolSpec, TurnRequest};
use crate::tools::{Effect, Outcome, Tool, Workspace, names};
use crate::transcript::{Block, Message};

struct Echo;

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec { name: "Echo".into(), description: "Says its text back.".into(), schema: json!({"type": "object"}) }
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        Vec::new()
    }

    fn summary(&self, input: &Value) -> String {
        input["text"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::ok(format!("echo: {}", input["text"].as_str().unwrap_or_default()))
    }
}

struct Bench {
    _dir: tempfile::TempDir,
    workspace: Workspace,
    control: Control,
    tools: Vec<Box<dyn Tool>>,
    events: Mutex<Vec<EngineEvent>>,
}

impl Bench {
    fn with(tools: Vec<Box<dyn Tool>>) -> Self {
        let dir = tempfile::tempdir().expect("dir");
        let workspace = Workspace::new(dir.path());
        let control = Control::new(domain::PermissionMode::Bypass, Arc::default());
        Self { _dir: dir, workspace, control, tools, events: Mutex::default() }
    }

    fn request(&self) -> TurnRequest {
        TurnRequest {
            model: "m".into(),
            system: "s".into(),
            messages: Vec::new(),
            tools: Agent::specs(&self.tools),
            effort: domain::Effort::Low,
            max_tokens: 1000,
        }
    }

    fn started(&self) -> Vec<domain::ToolCall> {
        let events = self.events.locked();
        events
            .iter()
            .filter_map(|event| match event {
                EngineEvent::ToolStarted(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }
}

fn result(block: &Block) -> (&str, bool) {
    match block {
        Block::ToolResult { content, is_error, .. } => (content.as_str(), *is_error),
        other => panic!("not a plain tool result: {other:?}"),
    }
}

#[test]
fn a_call_run_side_by_side_that_would_start_background_work_is_refused_instead_of_started() {
    let bench = Bench::with(vec![Box::new(crate::tools::subagent::AgentTool::default())]);
    let model = ScriptedModel::new([]);
    let emit = |event: EngineEvent| bench.events.locked().push(event);
    let agent = Agent {
        api: &model,
        tools: &bench.tools,
        workspace: &bench.workspace,
        control: &bench.control,
        emit: &emit,
        request: bench.request(),
        agents: &[],
        depth: 0,
        turn_limit: 5,
    };
    let run = ThisRun::default();
    let input = json!({"description": "Review", "prompt": "review", "run_in_background": true});
    let answered = agent.call_from("a1", names::AGENT, &input, &run, &[], None);
    let (content, failed) = result(&answered);
    assert!(failed && content.contains("cannot start background work"), "{content}");
    assert!(!run.background.busy(), "nothing was started");
    assert!(model.asked().is_empty(), "no sub-agent ran");
}

#[test]
fn the_input_a_hook_rewrote_is_the_one_that_runs_and_the_one_the_started_event_shows() {
    let mut bench = Bench::with(vec![Box::new(Echo)]);
    let rewrite = r#"printf '%s' '{"hookSpecificOutput":{"updatedInput":{"text":"safe"}}}'"#;
    bench.workspace.hooks = Hooks::parse(&json!({
        "PreToolUse": [{"matcher": "Echo", "hooks": [{"type": "command", "command": rewrite}]}]
    }))
    .expect("hooks");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "e1".into(),
            name: "Echo".into(),
            input: json!({"text": "unsafe"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]);
    let emit = |event: EngineEvent| bench.events.locked().push(event);
    let agent = Agent {
        api: &model,
        tools: &bench.tools,
        workspace: &bench.workspace,
        control: &bench.control,
        emit: &emit,
        request: bench.request(),
        agents: &[],
        depth: 0,
        turn_limit: 5,
    };
    let mut history = vec![Message::user("say it")];
    agent.run(&mut history).expect("run");

    let started = bench.started();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].input, Some(json!({"text": "safe"})), "the event carries the input that ran");
    assert_eq!(started[0].summary, "safe");
    let sent = history[1].tool_uses().map(|(_, _, input)| input.clone()).collect::<Vec<_>>();
    assert_eq!(sent, [json!({"text": "unsafe"})], "the saved call stays what the model sent");
    assert_eq!(result(&history[2].blocks[0]), ("echo: safe", false));
}
