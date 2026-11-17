use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::mcp::{McpServer, ServerSpec};
use crate::model::http::{Http, NetHttp};
use crate::tools::{Outcome, Tool, Workspace};

const SERVER: &str = r#"
echo $$ > "$2"
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$0"
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  said() { printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"%s"}]}}\n' "$id" "$1"; }
  case "$line" in
    *'"method":"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"capabilities":{}}}\n' "$id";;
    *'"method":"tools/list"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"tools":[{"name":"slow","inputSchema":{"type":"object"}},{"name":"quick","inputSchema":{"type":"object"}}]}}\n' "$id";;
    *'"name":"slow"'*) : > "$1"; SLOW; said late;;
    *'"name":"quick"'*) said quick;;
  esac
done
"#;

struct Fixture {
    dir: tempfile::TempDir,
    server: Arc<McpServer>,
}

impl Fixture {
    fn running(script: &str) -> Self {
        let dir = tempfile::tempdir().expect("dir");
        let at = |name: &str| dir.path().join(name).display().to_string();
        let spec = ServerSpec::Stdio {
            name: "fixture".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), script.into(), at("log"), at("started"), at("pid")],
            env: vec![],
        };
        let http: Arc<dyn Http> = Arc::new(NetHttp);
        let server = Arc::new(McpServer::open(&spec, &http).expect("open"));
        Self { dir, server }
    }

    fn pid(&self) -> u32 {
        std::fs::read_to_string(self.dir.path().join("pid")).expect("pid").trim().parse().expect("a pid")
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("log")).unwrap_or_default()
    }

    fn cancelled_slow_call(&self) -> Vec<Box<dyn Tool>> {
        let mut tools: Vec<Box<dyn Tool>> =
            self.server.tools().expect("tools").into_iter().map(|tool| Box::new(tool) as Box<dyn Tool>).collect();
        let slow = tools.remove(0);
        let workspace = Arc::new(Workspace::new(self.dir.path()));
        let running = Arc::clone(&workspace);
        let call = std::thread::spawn(move || slow.run(&json!({}), &running));
        until("the slow call started", || self.dir.path().join("started").exists());
        workspace.stop.store(true, Ordering::Relaxed);
        assert!(call.join().expect("joined").is_error);
        tools
    }
}

fn until(what: &str, met: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !met() {
        assert!(Instant::now() < deadline, "never happened: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn gone(pid: u32) -> bool {
    !platform::alive(pid)
}

#[test]
fn a_server_let_go_at_the_end_of_a_run_leaves_no_zombie() {
    let fixture = Fixture::running(&SERVER.replace("SLOW", "sleep 0.1"));
    fixture.server.initialize_stopped(&AtomicBool::new(false)).expect("initialize");
    let pid = fixture.pid();
    assert!(platform::alive(pid));
    let Fixture { dir, server } = fixture;
    drop(server);
    until("the server's process is gone, not a zombie", || gone(pid));
    drop(dir);
}

#[test]
fn a_server_that_dies_as_it_starts_says_why_in_its_own_words() {
    let fixture = Fixture::running("echo 'fatal: MCP_API_KEY is not set' >&2; exit 1");
    let error = fixture.server.initialize_stopped(&AtomicBool::new(false)).expect_err("it never answers");
    assert!(error.contains("fixture stopped"), "{error}");
    assert!(error.contains("MCP_API_KEY is not set"), "{error}");
}

#[test]
fn cancelling_one_call_tells_the_server_and_leaves_its_other_tools_working() {
    let fixture = Fixture::running(&SERVER.replace("SLOW", "sleep 0.2"));
    let tools = fixture.cancelled_slow_call();
    let quick = tools[0].run(&json!({}), &Workspace::new(fixture.dir.path()));
    assert_eq!(quick, Outcome::ok("quick"), "the server outlived the cancelled call");
    let log = fixture.log();
    assert!(log.contains(r#""method":"notifications/cancelled""#), "{log}");
    assert!(log.contains(r#""requestId":3"#), "{log}");
    assert!(platform::alive(fixture.pid()));
}

#[test]
fn a_server_that_says_nothing_after_a_cancelled_call_is_ended() {
    let fixture = Fixture::running(&SERVER.replace("SLOW", "sleep 60"));
    let tools = fixture.cancelled_slow_call();
    let pid = fixture.pid();
    until("the silent server is ended", || gone(pid));
    let after = tools[0].run(&json!({}), &Workspace::new(fixture.dir.path()));
    assert!(after.is_error && after.content.contains("fixture stopped"), "{after:?}");
}

#[test]
fn a_line_longer_than_the_limit_is_an_error_and_is_not_read_whole() {
    let endless = "read -r line; head -c 17000000 /dev/zero | tr '\\0' 'a'; echo; sleep 60";
    let fixture = Fixture::running(endless);
    let started = Instant::now();
    let error = fixture.server.initialize_stopped(&AtomicBool::new(false)).expect_err("too long");
    assert!(error.contains("longer than 16 MiB"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(60), "{:?}", started.elapsed());
}
