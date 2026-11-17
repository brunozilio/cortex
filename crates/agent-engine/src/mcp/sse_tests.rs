use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::sync::mpsc::{Sender, channel};

use crate::mcp::{McpServer, ServerSpec};
use crate::model::http::{Body, Http, Response};
use crate::model::{BoxFuture, CancellationToken, ModelError};
use crate::tools::{Outcome, Tool, Workspace};

type Posted = (String, Vec<(String, String)>);
type Chunk = Result<Vec<u8>, String>;

struct Legacy {
    endpoint: Mutex<String>,
    stream: Mutex<Option<Sender<Chunk>>>,
    posted: Mutex<Vec<Posted>>,
    methods: Mutex<Vec<String>>,
    down: AtomicBool,
    silent: AtomicBool,
}

impl Legacy {
    fn new(endpoint: &str) -> Arc<Self> {
        Arc::new(Self {
            endpoint: Mutex::new(endpoint.into()),
            stream: Mutex::default(),
            posted: Mutex::default(),
            methods: Mutex::default(),
            down: AtomicBool::new(false),
            silent: AtomicBool::new(false),
        })
    }

    fn drop_the_stream(&self) {
        self.down.store(true, Ordering::Relaxed);
        self.stream.lock().expect("stream").take();
    }

    fn methods(&self) -> Vec<String> {
        self.methods.lock().expect("methods").clone()
    }

    fn send(&self, event: &str, data: &str) {
        if let Some(stream) = self.stream.lock().expect("stream").as_ref() {
            let _ = stream.try_send(Ok(format!("event: {event}\ndata: {data}\n\n").into_bytes()));
        }
    }

    fn open_stream(&self) -> Result<Response, ModelError> {
        if self.down.load(Ordering::Relaxed) {
            return Err(ModelError::Network("the server is down".into()));
        }
        let (send, receive) = channel(64);
        *self.stream.lock().expect("stream") = Some(send);
        let endpoint = self.endpoint.lock().expect("endpoint").clone();
        self.send("endpoint", &endpoint);
        let body = Body::chunks(receive);
        Ok(Response { status: 200, headers: vec![("content-type".into(), "text/event-stream".into())], body })
    }
}

impl Http for Legacy {
    fn get_stream<'a>(
        &'a self,
        _url: &'a str,
        _headers: &'a [(String, String)],
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Response, ModelError>> {
        Box::pin(std::future::ready(self.open_stream()))
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        self.posted.lock().expect("posted").push((url.into(), headers.to_vec()));
        let message: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        if let Some(method) = message["method"].as_str() {
            self.methods.lock().expect("methods").push(method.to_owned());
        }
        if let Some(id) = message.get("id").filter(|_| !self.silent.load(Ordering::Relaxed)) {
            let result = match message["method"].as_str() {
                Some("tools/list") => json!({ "tools": [{ "name": "echo", "inputSchema": { "type": "object" } }] }),
                Some("tools/call") => {
                    json!({ "content": [{ "type": "text", "text": message["params"]["arguments"]["say"] }] })
                }
                _ => json!({ "capabilities": { "tools": {} } }),
            };
            self.send("message", &json!({ "jsonrpc": "2.0", "id": "srv-1", "method": "ping" }).to_string());
            self.send("message", &json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string());
        }
        Ok(Response { status: 202, headers: Vec::new(), body: Body::from(Vec::new()) })
    }
}

fn spec() -> ServerSpec {
    ServerSpec::Sse {
        name: "legacy".into(),
        url: "https://mcp.test/sse".into(),
        headers: vec![("Authorization".into(), "Bearer t".into())],
    }
}

#[test]
fn an_sse_server_is_posted_to_where_its_stream_says_and_answers_on_the_stream() {
    let legacy = Legacy::new("/messages?session=1");
    let http: Arc<dyn Http> = legacy.clone();
    let server = Arc::new(McpServer::open(&spec(), &http).expect("connected"));
    let tools = server.tools().expect("tools");
    assert_eq!(tools[0].spec().name, "mcp__legacy__echo");
    assert_eq!(tools[0].run(&json!({ "say": "hello" }), &Workspace::new("/")), Outcome::ok("hello"));
    let posted = legacy.posted.lock().expect("posted").clone();
    assert!(posted.iter().all(|(url, _)| url == "https://mcp.test/messages?session=1"), "{posted:?}");
    assert!(posted.iter().all(|(_, headers)| headers.contains(&("Authorization".into(), "Bearer t".into()))));
    let answered_ping = posted.len() > 4;
    assert!(answered_ping, "the server's pings are answered: {posted:?}");
}

#[test]
fn an_endpoint_on_another_site_is_refused() {
    let http: Arc<dyn Http> = Legacy::new("https://elsewhere.test/steal");
    let error = McpServer::open(&spec(), &http).expect_err("refused").to_string();
    assert!(error.contains("another site"), "{error}");
}

#[test]
fn a_request_after_the_stream_ended_fails_instead_of_waiting() {
    let legacy = Legacy::new("/messages");
    let http: Arc<dyn Http> = legacy.clone();
    let server = McpServer::open(&spec(), &http).expect("connected");
    legacy.drop_the_stream();
    let started = std::time::Instant::now();
    let error = server.initialize_stopped(&platform::Stop::new()).expect_err("closed").to_string();
    assert!(error.contains("closed its stream"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(60), "{:?}", started.elapsed());
}

fn until(what: &str, met: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !met() {
        assert!(std::time::Instant::now() < deadline, "never happened: {what}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_stream_that_dropped_is_opened_again_and_the_server_greeted_anew() {
    let legacy = Legacy::new("/messages?session=1");
    let http: Arc<dyn Http> = legacy.clone();
    let server = Arc::new(McpServer::open(&spec(), &http).expect("connected"));
    let tools = server.tools().expect("tools");
    let say = |text: &str| tools[0].run(&json!({ "say": text }), &Workspace::new("/"));
    assert_eq!(say("hello"), Outcome::ok("hello"));

    legacy.drop_the_stream();
    until("a call fails while the stream is down", || say("lost").content.contains("closed its stream"));
    let before = legacy.methods().len();
    *legacy.endpoint.lock().expect("endpoint") = "/messages?session=2".into();
    legacy.down.store(false, Ordering::Relaxed);
    until("the stream is back", || say("again") == Outcome::ok("again"));

    let after = legacy.methods().split_off(before);
    let greeted: Vec<&str> = after.iter().map(String::as_str).filter(|method| *method != "tools/call").collect();
    assert_eq!(greeted[..2], ["initialize", "notifications/initialized"], "{after:?}");
    let posted = legacy.posted.lock().expect("posted").clone();
    assert_eq!(posted.last().map(|(url, _)| url.as_str()), Some("https://mcp.test/messages?session=2"));
}

#[test]
fn a_request_in_flight_fails_when_the_stream_drops() {
    let legacy = Legacy::new("/messages");
    let http: Arc<dyn Http> = legacy.clone();
    let server = Arc::new(McpServer::open(&spec(), &http).expect("connected"));
    let tools = server.tools().expect("tools");
    legacy.silent.store(true, Ordering::Relaxed);
    let asking = std::thread::spawn(move || tools[0].run(&json!({ "say": "hello" }), &Workspace::new("/")));
    until("the call was posted", || legacy.methods().iter().any(|method| method == "tools/call"));
    legacy.drop_the_stream();
    let failed = asking.join().expect("joined");
    assert!(failed.is_error && failed.content.contains("closed its stream"), "{failed:?}");
}

#[test]
fn a_request_nobody_answers_gives_up_at_its_deadline() {
    use crate::mcp::Transport;
    let legacy = Legacy::new("/messages");
    let http: Arc<dyn Http> = legacy.clone();
    let never = platform::Stop::new();
    let mut transport = super::SseTransport::connect("legacy", "https://mcp.test/sse", &[], http, never.token(), None)
        .expect("connected");
    assert_eq!(transport.answer_wait, crate::mcp::ANSWER_WAIT, "the same wait as every other transport");
    transport.answer_wait = std::time::Duration::from_millis(100);
    legacy.silent.store(true, Ordering::Relaxed);
    let started = std::time::Instant::now();
    let error = transport
        .request_stopped(&json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/list" }), &never)
        .expect_err("nobody answered")
        .to_string();
    assert!(error.contains("legacy did not answer in"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(30), "{:?}", started.elapsed());
}

fn cancelled_soon(stop: &Arc<platform::Stop>) -> crossbeam_channel::Receiver<std::time::Instant> {
    let (marked, cancelled_at) = crossbeam_channel::unbounded();
    let stop = Arc::clone(stop);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = marked.send(std::time::Instant::now());
        stop.cancel();
    });
    cancelled_at
}

fn since_cancel(cancelled_at: &crossbeam_channel::Receiver<std::time::Instant>) -> std::time::Duration {
    let ended = std::time::Instant::now();
    let cancelled = cancelled_at.recv_timeout(std::time::Duration::from_secs(30)).expect("cancelled");
    ended.saturating_duration_since(cancelled)
}

#[test]
fn a_cancelled_run_stops_waiting_for_the_endpoint_at_once() {
    struct Silent(Mutex<Option<Sender<Chunk>>>);
    impl Http for Silent {
        fn get_stream<'a>(
            &'a self,
            _url: &'a str,
            _headers: &'a [(String, String)],
            _cancel: &'a CancellationToken,
        ) -> BoxFuture<'a, Result<Response, ModelError>> {
            let (send, receive) = channel(64);
            *self.0.lock().expect("stream") = Some(send);
            Box::pin(std::future::ready(Ok(Response { status: 200, headers: Vec::new(), body: Body::chunks(receive) })))
        }
        fn post(&self, _: &str, _: &[(String, String)], _: &str) -> Result<Response, String> {
            Err("unused".into())
        }
    }
    let silent = Arc::new(Silent(Mutex::default()));
    let http: Arc<dyn Http> = silent.clone();
    let stop = Arc::new(platform::Stop::new());
    let cancelled_at = cancelled_soon(&stop);
    let error = McpServer::open_stopped(&spec(), &http, &stop).expect_err("gave up").to_string();
    let took = since_cancel(&cancelled_at);
    eprintln!("cancel latency, SSE server naming its endpoint: {took:?}");
    assert!(error.contains("in time"), "{error}");
    assert!(took < std::time::Duration::from_secs(1), "{took:?}");
    drop(silent.0.lock().expect("stream").take());
}

#[test]
fn a_cancelled_run_stops_waiting_for_an_answer_on_the_stream_at_once() {
    use crate::mcp::Transport;
    let legacy = Legacy::new("/messages");
    let http: Arc<dyn Http> = legacy.clone();
    let transport =
        super::SseTransport::connect("legacy", "https://mcp.test/sse", &[], http, &CancellationToken::new(), None)
            .expect("connected");
    legacy.silent.store(true, Ordering::Relaxed);
    let stop = Arc::new(platform::Stop::new());
    let cancelled_at = cancelled_soon(&stop);
    let error = transport
        .request_stopped(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }), &stop)
        .expect_err("cancelled");
    let took = since_cancel(&cancelled_at);
    eprintln!("cancel latency, SSE request waiting for its answer: {took:?}");
    assert_eq!(error, crate::mcp::McpError::Cancelled);
    assert!(took < std::time::Duration::from_secs(1), "{took:?}");
    assert!(transport.waiting.lock().expect("waiting").is_empty(), "the cancelled request is forgotten");
}

#[test]
fn closing_an_sse_server_lets_its_stream_go_at_once() {
    use crate::mcp::Transport;
    let legacy = Legacy::new("/messages");
    let http: Arc<dyn Http> = legacy.clone();
    let transport =
        super::SseTransport::connect("legacy", "https://mcp.test/sse", &[], http, &CancellationToken::new(), None)
            .expect("connected");
    let streaming = || legacy.stream.lock().expect("stream").as_ref().is_some_and(|stream| !stream.is_closed());
    assert!(streaming());
    let closed = std::time::Instant::now();
    transport.close();
    until("the stream was let go", || !streaming());
    let took = closed.elapsed();
    eprintln!("closing an SSE server let its stream go after {took:?}");
    assert!(took < std::time::Duration::from_secs(1), "{took:?}");
}
