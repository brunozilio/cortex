use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::inbox::Notice;
use domain::{EngineEvent, PermissionMode, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start};

#[test]
fn a_notice_reaches_the_model_with_the_next_tool_results() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.txt"), "hello").expect("file");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "r".into(),
            name: "Read".into(),
            input: json!({"file_path": "a.txt"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Seen.".into())]),
    ]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(&scripted)]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(dir.path().into(), "read it", model());
    request.policy.permission = PermissionMode::Bypass;
    request.policy.safety_check = false;
    request.host.credentials.client = 93_001;
    agent_engine::tools::inbox::push(93_001, Notice::new("monitor bash_9", "ERROR: build failed"));
    let events = finish(&start(&engine, &request));
    assert!(events.contains(&EngineEvent::Notified(vec![Notice::new("monitor bash_9", "ERROR: build failed")])));
    let second = format!("{:?}", scripted.asked()[1].messages);
    assert!(second.contains("monitor bash_9") && second.contains("ERROR: build failed"), "{second}");
    assert!(agent_engine::tools::inbox::take(93_001).is_empty(), "taken by the run");
}

#[cfg(unix)]
const ALLOWING_CHANNEL: &str = r#"
read init
printf '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"experimental":{"claude/channel":{},"claude/channel/permission":{}}}}}\n'
read initialized
while read line; do
  case "$line" in
    *permission_request*)
      id=$(printf '%s' "$line" | sed 's/.*"request_id":"\([a-z]*\)".*/\1/')
      printf '{"jsonrpc":"2.0","method":"notifications/claude/channel/permission","params":{"request_id":"%s","behavior":"allow"}}\n' "$id" ;;
  esac
done
"#;

#[cfg(unix)]
#[test]
fn a_channel_that_takes_permission_prompts_can_allow_a_call_while_the_terminal_waits() {
    let spec = agent_engine::mcp::ServerSpec::Stdio {
        name: "phone".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), ALLOWING_CHANNEL.into()],
        env: vec![],
        sandbox: Default::default(),
    };
    let http: Arc<dyn agent_engine::model::http::Http> = Arc::new(agent_engine::model::http::ScriptedHttp::new([]));
    let _channel = agent_engine::mcp::channel::open(&spec, &http, 93_002).expect("channel");
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "w".into(),
            name: "Write".into(),
            input: json!({"file_path": "out.txt", "content": "from the phone"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Written.".into())]),
    ]));
    let engine =
        Engine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(&scripted)]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(dir.path().into(), "write it", model());
    request.policy.permission = PermissionMode::Ask;
    request.policy.safety_check = false;
    request.host.credentials.client = 93_002;
    let events = finish(&start(&engine, &request));
    assert!(events.iter().any(|event| matches!(event, EngineEvent::Asked(_))), "the terminal was asked too");
    assert!(
        events.iter().any(|event| matches!(event, EngineEvent::Answered { decision: domain::Decision::Allow, .. })),
        "{events:?}"
    );
    assert_eq!(std::fs::read_to_string(dir.path().join("out.txt")).expect("written"), "from the phone");
}
