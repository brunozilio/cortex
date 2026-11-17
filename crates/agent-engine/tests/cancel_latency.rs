#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender, unbounded};
use std::time::{Duration, Instant};

use agent_engine::model::http::NetHttp;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::model::wires::chat::{Chat, Endpoint, Endpoints};
use agent_engine::model::{BoxFuture, CancellationToken, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use agent_engine::{AgentEngine, ApiSource, Engine, RunHandle};
use domain::{EngineEvent, ModelOption, PermissionMode, PromptRequest, RunId};
use serde_json::json;

const PATIENCE: Duration = Duration::from_secs(20);
const PROMPT_CANCEL: Duration = Duration::from_millis(200);

struct One(Arc<dyn ModelApi>);

impl ApiSource for One {
    fn api(&self, _request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        Ok(Arc::clone(&self.0))
    }
}

struct Project {
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        static INSTALLED: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        let installed = INSTALLED.get_or_init(|| tempfile::tempdir().expect("installed content"));
        let _ = content::use_folder(installed.path().to_path_buf());
        Self { home: tempfile::tempdir().expect("home"), repo: tempfile::tempdir().expect("repo") }
    }

    fn servers(&self, servers: &[(&str, String)]) {
        let folder = self.repo.path().join(brand::DIR);
        std::fs::create_dir_all(&folder).expect("project folder");
        let mut config = serde_json::Map::new();
        for (name, script) in servers {
            let path = self.repo.path().join(format!("{name}.sh"));
            std::fs::write(&path, script).expect("server script");
            config.insert((*name).to_owned(), json!({"command": "/bin/sh", "args": [path.display().to_string()]}));
        }
        let names: Vec<&str> = servers.iter().map(|(name, _)| *name).collect();
        std::fs::write(folder.join("mcp.json"), json!({ "mcpServers": config }).to_string()).expect("mcp.json");
        std::fs::write(folder.join("settings.local.json"), json!({ "enabledMcpServers": names }).to_string())
            .expect("approval");
    }

    fn request(&self) -> PromptRequest {
        let mut request = PromptRequest::plain(self.repo.path().into(), "go", ModelOption::new("m", "M"));
        request.policy.permission = PermissionMode::Bypass;
        request.host.extensions = Some(domain::Extensions {
            home: Some(self.home.path().into()),
            trust: domain::trust::Trust::Trusted,
            ..Default::default()
        });
        request
    }
}

fn server(tool: &str, on_call: &str) -> String {
    format!(
        r#"while IFS= read -r line; do
case "$line" in
*'"method":"initialize"'*) printf '%s\n' '{{"jsonrpc":"2.0","id":1,"result":{{}}}}';;
*'"method":"tools/list"'*) printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"tools":[{{"name":"{tool}","inputSchema":{{"type":"object"}}}}]}}}}';;
*'"method":"tools/call"'*) {on_call};;
esac
done"#
    )
}

fn start(engine: &Engine, request: &PromptRequest) -> (Receiver<EngineEvent>, Box<dyn RunHandle>) {
    let (send, receive) = unbounded();
    let send = Mutex::new(send);
    let sink = Arc::new(move |_run: RunId, event: EngineEvent| {
        let _ = send.lock().expect("send").send(event);
    });
    (receive, engine.start(RunId::new(), request, sink).expect("start"))
}

fn ended_within(events: &Receiver<EngineEvent>, cancelled: Instant) -> (Duration, Vec<EngineEvent>) {
    let mut seen = Vec::new();
    while let Ok(event) = events.recv_timeout(PATIENCE) {
        let terminal = event.is_terminal();
        seen.push(event);
        if terminal {
            return (cancelled.elapsed(), seen);
        }
    }
    panic!("the run never ended: {seen:?}");
}

fn stalled_stream() -> (String, Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let (streaming, heard) = unbounded();
    std::thread::spawn(move || {
        let Ok((socket, _)) = listener.accept() else { return };
        let mut reader = BufReader::new(socket);
        let mut length = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            if line == "\r\n" {
                break;
            }
        }
        let mut body = vec![0; length];
        let _ = reader.read_exact(&mut body);
        let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";
        let _ = reader.get_mut().write_all(head);
        let _ = reader.get_mut().flush();
        let _ = streaming.send(());
        let mut byte = [0];
        let _ = reader.read(&mut byte);
    });
    (format!("http://{address}"), heard)
}

struct Local(String);

impl Endpoints for Local {
    fn endpoint(&self, model: &str) -> Result<Endpoint, ModelError> {
        Ok(Endpoint { base_url: self.0.clone(), model: model.to_owned() })
    }
}

#[test]
fn cancelling_a_run_whose_model_stream_has_stalled_ends_it_at_once() {
    let project = Project::new();
    let (base, streaming) = stalled_stream();
    let chat: Arc<dyn ModelApi> = Arc::new(Chat::new(Arc::new(NetHttp), Arc::new(Local(base))).without_backoff());
    let engine = Engine::new(Arc::new(One(chat)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &project.request());
    streaming.recv_timeout(PATIENCE).expect("the model stream opened");
    std::thread::sleep(Duration::from_millis(100));
    let cancelled = Instant::now();
    handle.cancel();
    let (took, seen) = ended_within(&events, cancelled);
    eprintln!("cancel latency, stalled model stream: {took:?}");
    assert!(matches!(seen.last(), Some(EngineEvent::Cancelled)), "{seen:?}");
    assert!(took < PROMPT_CANCEL, "cancel took {took:?}");
}

fn safety_check(request: &TurnRequest) -> Option<Result<Reply, ModelError>> {
    let checking = request.messages.iter().any(|message| message.text().contains("<transcript>"));
    checking.then(|| ScriptedModel::reply(vec![Block::Text("<severity>0</severity>".into())]))
}

struct CallsTool {
    asked: AtomicUsize,
    tool: &'static str,
}

impl ModelApi for CallsTool {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _on: OnDelta<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(std::future::ready(self.answer(request)))
    }
}

impl CallsTool {
    fn answer(&self, request: &TurnRequest) -> Result<Reply, ModelError> {
        if let Some(verdict) = safety_check(request) {
            return verdict;
        }
        match self.asked.fetch_add(1, Ordering::SeqCst) {
            0 => {
                ScriptedModel::reply(vec![Block::ToolUse { id: "t1".into(), name: self.tool.into(), input: json!({}) }])
            }
            _ => ScriptedModel::reply(vec![Block::Text("done".into())]),
        }
    }
}

fn wait_for(path: &Path) {
    let until = Instant::now() + PATIENCE;
    while !path.exists() {
        assert!(Instant::now() < until, "{} never appeared", path.display());
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
#[cfg(unix)]
fn cancelling_a_run_whose_mcp_call_has_stalled_ends_it_at_once() {
    let project = Project::new();
    let started = project.home.path().join("started");
    let call = format!("touch '{}'; sleep 30", started.display());
    project.servers(&[("slow", server("wait", &call))]);
    let model: Arc<dyn ModelApi> = Arc::new(CallsTool { asked: AtomicUsize::new(0), tool: "mcp__slow__wait" });
    let engine = Engine::new(Arc::new(One(model)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &project.request());
    wait_for(&started);
    std::thread::sleep(Duration::from_millis(100));
    let cancelled = Instant::now();
    handle.cancel();
    let (took, seen) = ended_within(&events, cancelled);
    eprintln!("cancel latency, stalled MCP call: {took:?}");
    assert!(matches!(seen.last(), Some(EngineEvent::Cancelled)), "{seen:?}");
    assert!(took < PROMPT_CANCEL, "cancel took {took:?}");
}

fn http_mcp(started: Sender<()>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    std::thread::spawn(move || {
        for socket in listener.incoming().map_while(Result::ok) {
            let started = started.clone();
            std::thread::spawn(move || answer_mcp_over_http(socket, &started));
        }
    });
    format!("http://{address}/mcp")
}

fn answer_mcp_over_http(socket: std::net::TcpStream, started: &Sender<()>) {
    let mut reader = BufReader::new(socket);
    loop {
        let mut length = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            if line == "\r\n" {
                break;
            }
        }
        let mut body = vec![0; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let asked: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        let result = match asked["method"].as_str() {
            Some("tools/call") => {
                let _ = started.send(());
                let mut byte = [0];
                let _ = reader.read(&mut byte);
                return;
            }
            Some("tools/list") => json!({"tools": [{"name": "wait", "inputSchema": {"type": "object"}}]}),
            _ => json!({}),
        };
        let answer = match asked.get("id") {
            Some(id) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
            None => String::new(),
        };
        let status = if answer.is_empty() { "202 Accepted" } else { "200 OK" };
        let head =
            format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", answer.len());
        let socket = reader.get_mut();
        if socket.write_all(head.as_bytes()).and_then(|()| socket.write_all(answer.as_bytes())).is_err() {
            return;
        }
    }
}

#[test]
fn cancelling_a_run_whose_mcp_call_over_http_has_stalled_ends_it_at_once() {
    let project = Project::new();
    let (started, called) = unbounded();
    let url = http_mcp(started);
    let folder = project.repo.path().join(brand::DIR);
    std::fs::create_dir_all(&folder).expect("project folder");
    let config = json!({"mcpServers": {"remote": {"type": "http", "url": url}}});
    std::fs::write(folder.join("mcp.json"), config.to_string()).expect("mcp.json");
    std::fs::write(folder.join("settings.local.json"), json!({"enabledMcpServers": ["remote"]}).to_string())
        .expect("approval");
    let model: Arc<dyn ModelApi> = Arc::new(CallsTool { asked: AtomicUsize::new(0), tool: "mcp__remote__wait" });
    let engine = Engine::new(Arc::new(One(model)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &project.request());
    called.recv_timeout(PATIENCE).expect("the call reached the server");
    std::thread::sleep(Duration::from_millis(100));
    let cancelled = Instant::now();
    handle.cancel();
    let (took, seen) = ended_within(&events, cancelled);
    eprintln!("cancel latency, stalled MCP call over HTTP: {took:?}");
    assert!(matches!(seen.last(), Some(EngineEvent::Cancelled)), "{seen:?}");
    assert!(took < PROMPT_CANCEL, "cancel took {took:?}");
}

#[test]
#[cfg(unix)]
fn cancelling_a_run_whose_command_hook_is_still_running_ends_it_and_the_hook_at_once() {
    let project = Project::new();
    let started = project.home.path().join("hook-started");
    let hook = format!("echo $$ > '{path}.pid'; touch '{path}'; exec sleep 30", path = started.display());
    let settings = json!({"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": hook}]}]}});
    let folder = project.home.path().join(brand::DIR);
    std::fs::create_dir_all(&folder).expect("settings folder");
    std::fs::write(folder.join("settings.json"), settings.to_string()).expect("settings");
    let model: Arc<dyn ModelApi> =
        Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("done".into())])]));
    let engine = Engine::new(Arc::new(One(model)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &project.request());
    wait_for(&started);
    std::thread::sleep(Duration::from_millis(100));
    let cancelled = Instant::now();
    handle.cancel();
    let (took, seen) = ended_within(&events, cancelled);
    eprintln!("cancel latency, running command hook: {took:?}");
    assert!(matches!(seen.last(), Some(EngineEvent::Cancelled)), "{seen:?}");
    assert!(took < PROMPT_CANCEL, "cancel took {took:?}");
    let pid_file = format!("{}.pid", started.display());
    let pid: u32 = std::fs::read_to_string(pid_file).expect("pid").trim().parse().expect("a pid");
    assert!(!platform::alive(pid), "the hook's process {pid} was ended with the run");
}

struct Stalls {
    asked: AtomicUsize,
    reached: Mutex<Sender<&'static str>>,
}

impl ModelApi for Stalls {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            if let Some(verdict) = safety_check(request) {
                return verdict;
            }
            let who = if request.system.contains("# You are a sub-agent") {
                "sub-agent"
            } else {
                if self.asked.fetch_add(1, Ordering::SeqCst) == 0 {
                    let input = json!({"description": "Look around", "prompt": "look", "run_in_background": true});
                    return ScriptedModel::reply(vec![Block::ToolUse { id: "a1".into(), name: "Agent".into(), input }]);
                }
                "main"
            };
            let _ = self.reached.lock().expect("reached").send(who);
            cancel.cancelled().await;
            Err(ModelError::Cancelled)
        })
    }
}

fn threads() -> usize {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_dir("/proc/self/task").map(Iterator::count).unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let listed =
            std::process::Command::new("ps").args(["-M", "-p", &std::process::id().to_string()]).output().expect("ps");
        String::from_utf8_lossy(&listed.stdout).lines().count().saturating_sub(1)
    }
}

#[test]
fn cancelling_a_run_with_a_background_sub_agent_waiting_on_its_model_ends_both_at_once() {
    let project = Project::new();
    let (reached, heard) = unbounded();
    let model: Arc<dyn ModelApi> = Arc::new(Stalls { asked: AtomicUsize::new(0), reached: Mutex::new(reached) });
    let engine = Engine::new(Arc::new(One(model)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &project.request());
    let mut waiting = vec![heard.recv_timeout(PATIENCE).expect("one"), heard.recv_timeout(PATIENCE).expect("two")];
    waiting.sort_unstable();
    assert_eq!(waiting, ["main", "sub-agent"]);
    std::thread::sleep(Duration::from_millis(100));
    let cancelled = Instant::now();
    handle.cancel();
    let (took, seen) = ended_within(&events, cancelled);
    eprintln!("cancel latency, run with a background sub-agent: {took:?}");
    assert!(matches!(seen.last(), Some(EngineEvent::Cancelled)), "{seen:?}");
    assert!(took < PROMPT_CANCEL, "cancel took {took:?}");
}

#[test]
#[ignore = "a measurement: run with --ignored --nocapture"]
#[cfg(unix)]
fn threads_during_a_run_with_three_mcp_servers_and_a_background_agent() {
    let project = Project::new();
    let ping = "printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"content\":[]}}'";
    project.servers(&[("one", server("ping", ping)), ("two", server("ping", ping)), ("three", server("ping", ping))]);
    let (reached, heard) = unbounded();
    let model: Arc<dyn ModelApi> = Arc::new(Stalls { asked: AtomicUsize::new(0), reached: Mutex::new(reached) });
    let before = threads();
    let engine = Engine::new(Arc::new(One(model)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &project.request());
    let mut waiting = vec![heard.recv_timeout(PATIENCE).expect("one"), heard.recv_timeout(PATIENCE).expect("two")];
    waiting.sort_unstable();
    assert_eq!(waiting, ["main", "sub-agent"]);
    std::thread::sleep(Duration::from_millis(300));
    let during = threads();
    let cancelled = Instant::now();
    handle.cancel();
    let (took, _) = ended_within(&events, cancelled);
    eprintln!("threads before the engine: {before}, during the run: {during}, cancel took {took:?}");
}

#[test]
fn cancelling_a_run_waiting_for_the_user_s_answer_ends_it_at_once() {
    let project = Project::new();
    let mut request = project.request();
    request.policy.permission = PermissionMode::Ask;
    let input = json!({"file_path": project.repo.path().join("asked.txt"), "content": "x"});
    let model: Arc<dyn ModelApi> = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse { id: "w1".into(), name: "Write".into(), input }]),
        ScriptedModel::reply(vec![Block::Text("done".into())]),
    ]));
    let engine = Engine::new(Arc::new(One(model)), Arc::new(InMemorySessions::default()));
    let (events, mut handle) = start(&engine, &request);
    let until = Instant::now() + PATIENCE;
    loop {
        let event = events.recv_timeout(PATIENCE).expect("the run asked");
        if matches!(event, EngineEvent::Asked(_)) {
            break;
        }
        assert!(Instant::now() < until && !event.is_terminal(), "{event:?}");
    }
    std::thread::sleep(Duration::from_millis(50));
    let cancelled = Instant::now();
    handle.cancel();
    let (took, seen) = ended_within(&events, cancelled);
    eprintln!("cancel latency, waiting for an answer: {took:?}");
    assert!(matches!(seen.last(), Some(EngineEvent::Cancelled)), "{seen:?}");
    assert!(took < PROMPT_CANCEL, "cancel took {took:?}");
    assert!(!project.repo.path().join("asked.txt").exists());
}

struct Where(Mutex<Vec<Option<String>>>);

impl ModelApi for Where {
    fn turn<'a>(
        &'a self,
        _request: &'a TurnRequest,
        _on: OnDelta<'a>,
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        self.0.lock().expect("seen").push(std::thread::current().name().map(str::to_owned));
        Box::pin(std::future::ready(ScriptedModel::reply(vec![Block::Text("here".into())])))
    }
}

#[test]
fn a_run_is_a_task_of_the_engine_s_one_runtime() {
    let project = Project::new();
    let model = Arc::new(Where(Mutex::default()));
    let engine = Engine::new(Arc::new(One(model.clone())), Arc::new(InMemorySessions::default()));
    for _ in 0..2 {
        let (events, _handle) = start(&engine, &project.request());
        let (_, seen) = ended_within(&events, Instant::now());
        assert!(matches!(seen.last(), Some(EngineEvent::Completed { .. })), "{seen:?}");
    }
    let seen = model.0.lock().expect("seen").clone();
    assert_eq!(seen.len(), 2);
    assert!(seen.iter().all(|name| name.as_deref() == Some(platform::runtime::BLOCKING_NAME)), "{seen:?}");
}
