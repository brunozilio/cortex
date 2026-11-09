//! Regression contracts for the native engine's migration, enforced in CI.
//! Run `--test migration_audit -- --nocapture` to print their evidence.
//! Fixtures use only temporary files, a scripted model and fake credentials.
use serde_json::{Value, json};
use std::sync::{Arc, mpsc};
use std::time::Duration;
use superfik_core::{EngineEvent, EngineKind, ModelOption, PromptRequest, RunId};
use superfik_engine::{AgentEngine, RunHandle};
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::model::{ModelApi, ModelError};
use superfik_native::sessions::InMemorySessions;
use superfik_native::tools::{Tool, Workspace};
use superfik_native::transcript::Block;
use superfik_native::{ApiSource, NativeEngine};

struct Source(Arc<ScriptedModel>);
impl ApiSource for Source {
    fn api(&self, _: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(self.0.clone())
    }
}
fn model() -> ModelOption {
    ModelOption::new(EngineKind::Native, "audit-model", "Audit")
}
fn engine(script: Arc<ScriptedModel>) -> NativeEngine {
    NativeEngine::new(Arc::new(Source(script)), Arc::new(InMemorySessions::default()))
}
fn start(engine: &NativeEngine, request: &PromptRequest) -> (Box<dyn RunHandle>, mpsc::Receiver<EngineEvent>) {
    let (tx, rx) = mpsc::channel();
    let sink = Arc::new(move |_: RunId, event: EngineEvent| {
        let _ = tx.send(event);
    });
    (engine.start(RunId::new(), request, sink).expect("start"), rx)
}
fn finish(rx: &mpsc::Receiver<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    loop {
        let event = rx.recv_timeout(Duration::from_secs(5)).expect("terminal event within five seconds");
        let terminal = event.is_terminal();
        events.push(event);
        if terminal {
            return events;
        }
    }
}
#[cfg(unix)]
fn bash_script(command: &str) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "command-1".into(),
            name: "Bash".into(),
            input: json!({"command":command}),
        }]),
        ScriptedModel::reply(vec![Block::Text("done".into())]),
    ]))
}

#[test]
#[cfg(unix)]
fn cancel_stops_the_running_shell_before_it_can_write_again() {
    use std::time::Instant;
    use superfik_core::PermissionMode;
    let dir = tempfile::tempdir().expect("dir");
    let script = bash_script("printf started > started; sleep 0.8; printf continued > after-cancel");
    let engine = engine(script);
    let mut request = PromptRequest::plain(dir.path().into(), "fixture", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;
    let (mut handle, rx) = start(&engine, &request);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !dir.path().join("started").exists() {
        assert!(Instant::now() < deadline, "shell started");
        std::thread::sleep(Duration::from_millis(10));
    }
    handle.cancel();
    let events = finish(&rx);
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Cancelled)));
    std::thread::sleep(Duration::from_millis(1100));
    let continued = dir.path().join("after-cancel").exists();
    println!("cancelled_event=true; file_written_after_cancel={continued}");
    assert!(!continued, "the shell continued to write after the UI reported cancellation");
}

#[test]
#[cfg(unix)]
fn shell_receives_only_the_request_github_identity() {
    use superfik_core::PermissionMode;
    let dir = tempfile::tempdir().expect("dir");
    let script = bash_script(
        "if [ \"$GH_TOKEN\" = audit-fixture-only ]; then printf request-identity-present; else printf request-identity-missing; fi",
    );
    let engine = engine(script);
    let mut request = PromptRequest::plain(dir.path().into(), "fixture", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;
    request.credentials.github = Some("audit-fixture-only".into());
    let (_handle, rx) = start(&engine, &request);
    let events = finish(&rx);
    let outputs: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::ToolResult { output, .. } => Some(output.as_str()),
            _ => None,
        })
        .collect();
    println!("safe_identity_check={outputs:?}");
    assert!(outputs.contains(&"request-identity-present"), "Bash did not use the identity supplied by the backend");
}

#[test]
fn context_overflow_compacts_and_continues() {
    let dir = tempfile::tempdir().expect("dir");
    let script = Arc::new(ScriptedModel::new([
        Err(ModelError::TooLong("audit context overflow".into())),
        ScriptedModel::reply(vec![Block::Text("summary".into())]),
        ScriptedModel::reply(vec![Block::Text("continued".into())]),
    ]));
    let engine = engine(script.clone());
    let (_handle, rx) = start(&engine, &PromptRequest::plain(dir.path().into(), "fixture", model()));
    let events = finish(&rx);
    println!(
        "model_calls={}; completed={}",
        script.asked().len(),
        events.iter().any(|e| matches!(e, EngineEvent::Completed { .. }))
    );
    assert!(
        events.iter().any(|e| matches!(e, EngineEvent::Completed { .. })),
        "context overflow ended the conversation without recovery"
    );
}

#[test]
fn native_completion_reports_spending_for_usage_governance() {
    let dir = tempfile::tempdir().expect("dir");
    let script = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("done".into())])]));
    let engine = engine(script);
    let (_handle, rx) = start(
        &engine,
        &PromptRequest::plain(
            dir.path().into(),
            "fixture",
            ModelOption::new(EngineKind::Native, "claude-sonnet-5-5", "Sonnet 5.5"),
        ),
    );
    let events = finish(&rx);
    let cost = events.iter().find_map(|e| match e {
        EngineEvent::Completed { cost_usd, .. } => Some(*cost_usd),
        _ => None,
    });
    println!("completion_cost={cost:?}");
    assert!(cost.flatten().is_some(), "the native engine reported no spending despite consuming fixture tokens");
}

#[test]
fn an_attached_png_can_be_read_as_visual_input() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let dir = tempfile::tempdir().expect("dir");
    let png = STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+nvXUAAAAASUVORK5CYII=")
        .expect("PNG fixture");
    std::fs::write(dir.path().join("reference.png"), png).expect("PNG");
    let outcome = superfik_native::tools::Read.run(&json!({"file_path":"reference.png"}), &Workspace::new(dir.path()));
    println!("image_read_failed={}; result={}", outcome.is_error, outcome.content);
    assert!(!outcome.is_error, "the attached visual reference cannot be passed to the model");
}

struct ImageServer;
impl superfik_native::mcp::Transport for ImageServer {
    fn request(&self, message: &Value) -> Result<Value, String> {
        let result = match message["method"].as_str() {
            Some("tools/list") => json!({"tools":[{"name":"screenshot","inputSchema":{"type":"object"}}]}),
            Some("tools/call") => {
                json!({"content":[{"type":"image","mimeType":"image/png","data":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6wAAAABJRU5ErkJggg=="}]})
            }
            _ => json!({}),
        };
        Ok(json!({"jsonrpc":"2.0","id":message["id"],"result":result}))
    }
    fn notify(&self, _: &Value) -> Result<(), String> {
        Ok(())
    }
}
#[test]
fn mcp_screenshot_is_not_silently_discarded() {
    let server = Arc::new(superfik_native::mcp::McpServer::new("audit", Box::new(ImageServer)));
    let tools = server.tools().expect("list");
    let result = tools[0].run(&json!({}), &Workspace::new("/"));
    println!("mcp_image_failed={}; content_length={}", result.is_error, result.content.len());
    assert!(!result.is_error);
    assert_eq!(result.images.len(), 1, "the MCP image disappeared from the tool result");
    assert_eq!(result.images[0].media_type, "image/png");
}

#[test]
fn subagent_schema_supports_background_execution() {
    let tool = superfik_native::tools::subagent::AgentTool::default();
    let schema = tool.spec().schema;
    println!("run_in_background_supported={}", schema["properties"].get("run_in_background").is_some());
    assert!(schema["properties"].get("run_in_background").is_some(), "only synchronous sub-agents are implemented");
}
