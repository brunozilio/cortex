//! Channels: MCP servers that push messages into a session (a chat bot, a
//! webhook) with `notifications/claude/channel`. A client opens the ones it
//! names (`<command> --channels telegram`); each stays up while the client
//! does, its messages go to the client's inbox, and runs reach its tools
//! (the replies) through a [`Bridge`] to that same server, never a second
//! one. A process, an SSE server or a streamable HTTP server can be one.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use domain::inbox::{CHANNEL_CAPABILITY, channel_notice};
use domain::mcp::{LocalMcp, McpEndpoint};
use serde_json::{Value, json};

use super::{Listener, McpServer, ServerSpec};
use crate::model::http::Http;
use crate::tools::inbox;

/// How long a channel may take to open.
const OPEN_WAIT: Duration = Duration::from_secs(30);

/// Opens `spec` as a channel of `client`: up, initialized, and declaring
/// itself a channel.
pub fn open(spec: &ServerSpec, http: &Arc<dyn Http>, client: u64) -> Result<Arc<McpServer>, String> {
    let name = spec.name().to_owned();
    let listener: Listener = Arc::new(move |method, params| {
        if let Some(notice) = channel_notice(&name, method, params) {
            inbox::push(client, notice);
        }
    });
    // A server that never answers gives up after OPEN_WAIT (its process ends).
    let (given_up, opened) = (AtomicBool::new(false), AtomicBool::new(false));
    let server = std::thread::scope(|scope| {
        scope.spawn(|| {
            let started = std::time::Instant::now();
            while !opened.load(Ordering::Relaxed) {
                if started.elapsed() > OPEN_WAIT {
                    given_up.store(true, Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        let server = McpServer::open_listening(spec, http, &given_up, Some(listener))
            .and_then(|server| server.initialize_stopped(&given_up).map(|()| server));
        opened.store(true, Ordering::Relaxed);
        server
    })
    .map_err(|error| {
        if given_up.load(Ordering::Relaxed) {
            format!("{} did not answer within {} s", spec.name(), OPEN_WAIT.as_secs())
        } else {
            error
        }
    })?;
    let declared = server.capabilities()["experimental"][CHANNEL_CAPABILITY].is_object();
    if !declared {
        return Err(format!("{} is not a channel (it does not declare {CHANNEL_CAPABILITY})", spec.name()));
    }
    Ok(Arc::new(server))
}

/// A run's way to an open channel's tools: the run's MCP messages, sent on
/// the channel's own connection.
#[derive(Debug)]
pub struct Bridge(pub Arc<McpServer>);

impl Bridge {
    pub fn local(server: &Arc<McpServer>) -> LocalMcp {
        LocalMcp::new(server.name(), Arc::new(Self(Arc::clone(server))))
    }
}

impl McpEndpoint for Bridge {
    fn handle(&self, message: &Value) -> Option<Value> {
        self.handle_stopped(message, &AtomicBool::new(false))
    }

    /// A cancelled run's call is cancelled at the channel, which stays up.
    fn handle_stopped(&self, message: &Value, stop: &AtomicBool) -> Option<Value> {
        let id = message.get("id")?.clone();
        let method = message["method"].as_str().unwrap_or_default();
        // The channel shook hands once, when it opened; each run only hears so.
        let answer = if method == "initialize" {
            Ok(json!({ "protocolVersion": super::PROTOCOL_VERSION, "capabilities": self.0.capabilities() }))
        } else {
            self.0.ask(method, message["params"].clone(), stop)
        };
        Some(match answer {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": error } }),
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::model::http::ScriptedHttp;

    /// A channel server in shell: it answers the handshake, then pushes a
    /// message; a `slow` call never answers, the others answer `sent` and
    /// how many calls were cancelled.
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
        };
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let server = open(&spec, &http, 9501).expect("channel");
        let deadline = Instant::now() + Duration::from_secs(5);
        let notices = loop {
            let notices = inbox::take(9501);
            if !notices.is_empty() || Instant::now() > deadline {
                break notices;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(notices, [domain::inbox::Notice::new("channel chat", "[chat_id=7]\ndeploy please")]);

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
        };
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let bridge = Bridge(open(&spec, &http, 9503).expect("channel"));
        let stop = AtomicBool::new(false);
        let slow = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "slow" } });
        let started = Instant::now();
        let dropped = std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                stop.store(true, Ordering::Relaxed);
            });
            bridge.handle_stopped(&slow, &stop).expect("answer")
        });
        assert!(started.elapsed() < Duration::from_secs(5), "the run stopped waiting");
        assert_eq!(dropped["id"], 4);
        assert!(dropped["error"]["message"].as_str().is_some_and(|error| error.contains("cancelled")), "{dropped}");

        let call = json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "reply" } });
        let replied = bridge.handle(&call).expect("answer");
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
        };
        let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
        let error = open(&spec, &http, 9502).expect_err("not a channel");
        assert!(error.contains("not a channel"), "{error}");
    }
}
