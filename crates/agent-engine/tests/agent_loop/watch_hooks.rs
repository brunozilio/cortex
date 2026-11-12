use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_engine::hooks::Hooks;
use agent_engine::model::ToolSpec;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::{Access, Outcome, Tool, Workspace};
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, PermissionMode};
use serde_json::{Value, json};

use crate::{Rig, tool_use};

#[derive(Debug, Clone)]
struct Issue {
    calls: Arc<AtomicUsize>,
}

impl Tool for Issue {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "mcp__jira__getIssue".into(),
            description: "Get an issue".into(),
            schema: json!({"type":"object"}),
        }
    }

    fn access(&self) -> Access {
        Access::Executes
    }

    fn summary(&self, input: &Value) -> String {
        input["key"].as_str().unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, _workspace: &Workspace) -> Outcome {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Outcome::ok(format!("token=abc123 for {}", input["key"].as_str().unwrap_or_default()))
    }

    fn kept(&self) -> Option<Arc<dyn Tool>> {
        Some(Arc::new(self.clone()))
    }
}

fn hooks(event: &str, command: &str) -> Hooks {
    Hooks::parse(
        &json!({event: [{"matcher": "mcp__jira__getIssue", "hooks": [{"type": "command", "command": command}]}]}),
    )
    .expect("hooks")
}

fn watched(client: u64, hooks: Hooks) -> (Vec<(bool, String)>, usize) {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.credentials.client = client;
    rig.workspace.hooks = hooks;
    let calls = Arc::new(AtomicUsize::new(0));
    let issue = Issue { calls: Arc::clone(&calls) };
    let watch = tool_use(
        "w1",
        "Watch",
        json!({ "tool": "mcp__jira__getIssue", "input": { "key": "KAN-1" }, "contains": "token", "every_seconds": 60 }),
    );
    let model =
        ScriptedModel::new([ScriptedModel::reply(vec![watch]), ScriptedModel::reply(vec![Block::Text("Ok.".into())])]);
    rig.run_with(&model, &mut vec![Message::user("watch it")], vec![Box::new(issue)]).expect("run");
    agent_engine::tools::watch::forget_client(client);
    let results = rig
        .events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::ToolResult { output, failed, .. } => Some((failed, output)),
            _ => None,
        })
        .collect();
    (results, calls.load(Ordering::Relaxed))
}

#[cfg(unix)]
#[test]
fn a_hook_that_blocks_the_watched_tool_blocks_the_watch_before_the_tool_is_called() {
    let (results, calls) = watched(9411, hooks("PreToolUse", "echo 'jira is off limits' >&2; exit 2"));

    assert!(results[0].0, "{results:?}");
    assert!(results[0].1.contains("Hook blocked mcp__jira__getIssue: jira is off limits"), "{results:?}");
    assert_eq!(calls, 0, "a blocked tool is never called");
}

#[cfg(unix)]
#[test]
fn a_hook_that_rewrites_the_watched_tool_s_output_hides_it_from_the_agent() {
    let output =
        json!({"hookSpecificOutput": {"hookEventName": "PostToolUse", "updatedToolOutput": "token=[redacted]"}});
    let (results, calls) = watched(9412, hooks("PostToolUse", &format!("printf '%s' '{output}'")));

    assert_eq!(calls, 1);
    assert!(!results[0].0 && results[0].1.starts_with("Already true on the first check"), "{results:?}");
    assert!(results[0].1.contains("token=[redacted]") && !results[0].1.contains("abc123"), "{results:?}");
}
