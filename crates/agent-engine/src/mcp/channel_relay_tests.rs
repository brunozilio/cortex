use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::{AskId, Decision, PermissionMode};
use serde_json::json;

use super::channel;
use super::channel_relay;
use super::servers::ServerSpec;
use crate::control::{Awaited, Control};
use crate::model::http::{Http, ScriptedHttp};

const ANSWERING: &str = r#"
read init
printf '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"experimental":{"claude/channel":{},"claude/channel/permission":{}}},"instructions":"Reply with the reply tool."}}\n'
read initialized
while read line; do
  case "$line" in
    *permission_request*)
      id=$(printf '%s' "$line" | sed 's/.*"request_id":"\([a-z]*\)".*/\1/')
      printf '{"jsonrpc":"2.0","method":"notifications/claude/channel/permission","params":{"request_id":"%s","behavior":"allow"}}\n' "$id" ;;
  esac
done
"#;

const SILENT: &str = r#"
read init
printf '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"experimental":{"claude/channel":{}}}}}\n'
read initialized
while read line; do :; done
"#;

fn spec(name: &str, script: &str) -> ServerSpec {
    ServerSpec::Stdio {
        name: name.into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![],
        sandbox: Default::default(),
    }
}

fn http() -> Arc<dyn Http> {
    Arc::new(ScriptedHttp::new([]))
}

fn within(waiting: &mut Awaited, wait: Duration) -> Option<Decision> {
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if let Ok(decision) = waiting.try_recv() {
            return Some(decision);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn a_permission_prompt_is_sent_to_the_channel_and_its_answer_settles_it() {
    let _server = channel::open(&spec("chat", ANSWERING), &http(), 9601).expect("channel");
    let control = Control::new(PermissionMode::Ask, Arc::default());
    let ask = AskId("ask-1".into());
    let mut waiting = control.open(ask.clone());
    let relayed =
        channel_relay::ask(9601, "call-1", "Bash", "Run ls", &json!({ "command": "ls" }), control.answerer(&ask));
    assert!(relayed.is_some(), "a channel that takes permission prompts is open");
    assert_eq!(within(&mut waiting, Duration::from_secs(60)), Some(Decision::Allow));
}

#[test]
fn a_channel_cannot_allow_a_call_to_its_own_tool_and_the_terminal_still_can_answer() {
    let _server = channel::open(&spec("chat", ANSWERING), &http(), 9602).expect("channel");
    let control = Control::new(PermissionMode::Ask, Arc::default());
    let ask = AskId("ask-2".into());
    let mut waiting = control.open(ask.clone());
    let _relayed = channel_relay::ask(
        9602,
        "call-2",
        "mcp__chat__reply",
        "Reply",
        &json!({ "text": "hi" }),
        control.answerer(&ask),
    )
    .expect("relayed");
    assert_eq!(within(&mut waiting, Duration::from_millis(800)), None, "the channel's own allow is ignored");
    control.answer(&ask, Decision::Deny);
    assert_eq!(within(&mut waiting, Duration::from_secs(5)), Some(Decision::Deny));
}

#[test]
fn a_channel_that_does_not_take_permission_prompts_is_not_asked() {
    let _server = channel::open(&spec("quiet", SILENT), &http(), 9603).expect("channel");
    let control = Control::new(PermissionMode::Ask, Arc::default());
    let ask = AskId("ask-3".into());
    let _waiting = control.open(ask.clone());
    assert!(channel_relay::ask(9603, "call-3", "Bash", "Run ls", &json!({}), control.answerer(&ask)).is_none());
}

#[test]
fn a_channel_s_instructions_reach_the_runs_it_serves() {
    let server = channel::open(&spec("chat", ANSWERING), &http(), 9604).expect("channel");
    assert_eq!(server.instructions().as_deref(), Some("Reply with the reply tool."));
    let bridge = channel::Bridge(Arc::clone(&server));
    let hello = domain::mcp::McpEndpoint::handle(
        &bridge,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
    )
    .expect("answer");
    assert_eq!(hello["result"]["instructions"], "Reply with the reply tool.");
}
