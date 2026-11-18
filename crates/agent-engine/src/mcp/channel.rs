use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use domain::channel_relay::permission_answer;
use domain::inbox::{CHANNEL_CAPABILITY, Channeled, channel_notice};
use domain::mcp::{LocalMcp, McpEndpoint};
use serde_json::{Value, json};

use super::{Listener, McpServer, ServerSpec};
use crate::model::http::Http;
use crate::tools::inbox;

const OPEN_WAIT: Duration = Duration::from_secs(30);

pub fn open(spec: &ServerSpec, http: &Arc<dyn Http>, client: u64) -> Result<Arc<McpServer>, String> {
    let name = spec.name().to_owned();
    let listener: Listener = Arc::new(move |method, params| {
        if let Some(Channeled { notice, dropped }) = channel_notice(&name, method, params) {
            if !dropped.is_empty() {
                tracing::warn!(channel = %name, ?dropped, "[channel] meta keys must be identifiers; these were left out");
            }
            inbox::push(client, notice);
        } else if let Some(relayed) = permission_answer(method, params) {
            super::channel_relay::answered(client, &name, relayed);
        }
    });
    let given_up = platform::Stop::new();
    let (opened, waiting) = crossbeam_channel::bounded::<()>(1);
    let server = std::thread::scope(|scope| {
        let given_up = &given_up;
        scope.spawn(move || {
            if let Err(crossbeam_channel::RecvTimeoutError::Timeout) = waiting.recv_timeout(OPEN_WAIT) {
                given_up.store(true, Ordering::Relaxed);
            }
        });
        let server = McpServer::open_listening(spec, http, given_up, Some(listener))
            .and_then(|server| server.initialize_stopped(given_up).map(|()| server));
        drop(opened);
        server
    })
    .map_err(|error| {
        if given_up.load(Ordering::Relaxed) {
            format!("{} did not answer within {} s", spec.name(), OPEN_WAIT.as_secs())
        } else {
            error.to_string()
        }
    })?;
    let declared = server.capabilities()["experimental"][CHANNEL_CAPABILITY].is_object();
    if !declared {
        return Err(format!("{} is not a channel (it does not declare {CHANNEL_CAPABILITY})", spec.name()));
    }
    let server = Arc::new(server);
    super::channel_relay::register(client, &server);
    Ok(server)
}

#[derive(Debug)]
pub struct Bridge(pub Arc<McpServer>);

impl Bridge {
    pub fn local(server: &Arc<McpServer>) -> LocalMcp {
        LocalMcp::new(server.name(), Arc::new(Self(Arc::clone(server))))
    }

    pub(super) fn answer(&self, message: &Value, stop: &platform::Stop) -> Option<Value> {
        let id = message.get("id")?.clone();
        let method = message["method"].as_str().unwrap_or_default();
        let answer = if method == "initialize" {
            let mut hello =
                json!({ "protocolVersion": super::PROTOCOL_VERSION, "capabilities": self.0.capabilities() });
            if let Some(instructions) = self.0.instructions() {
                hello["instructions"] = Value::String(instructions);
            }
            Ok(hello)
        } else {
            self.0.ask(method, message["params"].clone(), stop)
        };
        Some(match answer {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": error.to_string() } })
            }
        })
    }
}

impl McpEndpoint for Bridge {
    fn handle(&self, message: &Value) -> Option<Value> {
        self.answer(message, &platform::Stop::new())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::model::http::ScriptedHttp;

    const SERVER: &str = r#"
read init
id=$(printf '%s' "$init" | sed 's/.*"id":\([0-9]*\).*/\1/')
printf '{"jsonrpc":"2.0","id":%s,"result":{"capabilities":{"tools":{},"experimental":{"claude/channel":{}}}}}\n' "$id"
read initialized
printf '{"jsonrpc":"2.0","method":"notifications/claude/channel","params":{"content":"deploy please","meta":{"chat_id":"7"}}}\n'
cancelled=0
while read line; do
  case "$line" in
    *notifications/cancelled*) cancelled=$((cancelled+1)); continue ;;
    *'"slow"'*) continue ;;
  esac
  id=$(printf '%s' "$line" | sed 's/.*"id":\([0-9]*\).*/\1/')
  printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"sent"}],"cancelled":%s}}\n' "$id" "$cancelled"
done
"#;

    #[test]
    fn a_channel_s_messages_reach_the_inbox_and_runs_answer_through_the_same_server() {
        let spec = ServerSpec::Stdio {
            name: "chat".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), SERVER.into()],
            env: vec![],
            sandbox: Default::default(),
        };
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let server = open(&spec, &http, 9501).expect("channel");
        let deadline = Instant::now() + Duration::from_secs(120);
        let notices = loop {
            let notices = inbox::take(9501);
            if !notices.is_empty() || Instant::now() > deadline {
                break notices;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(notices.len(), 1);
        assert_eq!((notices[0].text.as_str(), notices[0].meta("chat_id")), ("deploy please", Some("7")));

        let bridge = Bridge(Arc::clone(&server));
        let hello = bridge.handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }));
        assert!(hello.expect("answer")["result"]["capabilities"]["experimental"]["claude/channel"].is_object());
        let call = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": { "name": "reply" } });
        let replied = bridge.handle(&call).expect("answer");
        assert_eq!(replied["id"], 9, "the run's own id comes back");
        assert_eq!(replied["result"]["content"][0]["text"], "sent");
    }

    #[test]
    fn a_cancelled_run_s_call_is_cancelled_at_the_channel_which_stays_up() {
        let spec = ServerSpec::Stdio {
            name: "chat".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), SERVER.into()],
            env: vec![],
            sandbox: Default::default(),
        };
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let local = Bridge::local(&open(&spec, &http, 9503).expect("channel"));
        let bridged = crate::mcp::LocalTransport::new(Arc::clone(&local.endpoint));
        let stop = platform::Stop::new();
        let slow = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "slow" } });
        let (dropped, took) = std::thread::scope(|scope| {
            let cancelled_at = scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                let cancelled = Instant::now();
                stop.cancel();
                cancelled
            });
            let dropped = crate::mcp::Transport::request_stopped(&bridged, &slow, &stop).expect("answer");
            let ended = Instant::now();
            (dropped, ended.saturating_duration_since(cancelled_at.join().expect("cancelled")))
        });
        eprintln!("cancel latency, call to a bridged channel: {took:?}");
        assert!(took < Duration::from_secs(1), "{took:?}");
        assert_eq!(dropped["id"], 4);
        assert!(dropped["error"]["message"].as_str().is_some_and(|error| error.contains("cancelled")), "{dropped}");

        let call = json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "reply" } });
        let replied = local.endpoint.handle(&call).expect("answer");
        assert_eq!(replied["result"]["content"][0]["text"], "sent", "the channel is still up");
        assert_eq!(replied["result"]["cancelled"], 1, "and was told the call was dropped");
        let _ = inbox::take(9503);
    }

    #[test]
    fn a_server_that_is_not_a_channel_is_refused() {
        let plain =
            "read l; printf '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{}}}\\n'; read l; sleep 5";
        let spec = ServerSpec::Stdio {
            name: "plain".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), plain.into()],
            env: vec![],
            sandbox: Default::default(),
        };
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let error = open(&spec, &http, 9502).expect_err("not a channel");
        assert!(error.contains("not a channel"), "{error}");
    }
}
