use std::io::{BufReader, Read};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::mcp::{McpServer, ServerSpec};
use crate::model::ModelError;
use crate::model::http::{Http, Response};
use crate::tools::{Outcome, Tool, Workspace};

type Posted = (String, Vec<(String, String)>);

struct Legacy {
    endpoint: String,
    stream: Mutex<Option<Sender<Vec<u8>>>>,
    posted: Mutex<Vec<Posted>>,
}

struct Pipe(Receiver<Vec<u8>>, Vec<u8>);

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.1.is_empty() {
            match self.0.recv_timeout(std::time::Duration::from_millis(25)) {
                Ok(bytes) => self.1 = bytes,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return Err(std::io::ErrorKind::TimedOut.into());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
        let n = buf.len().min(self.1.len());
        buf[..n].copy_from_slice(&self.1[..n]);
        self.1.drain(..n);
        Ok(n)
    }
}

impl Legacy {
    fn new(endpoint: &str) -> Arc<Self> {
        Arc::new(Self { endpoint: endpoint.into(), stream: Mutex::default(), posted: Mutex::default() })
    }

    fn send(&self, event: &str, data: &str) {
        if let Some(stream) = self.stream.lock().expect("stream").as_ref() {
            let _ = stream.send(format!("event: {event}\ndata: {data}\n\n").into_bytes());
        }
    }
}

impl Http for Legacy {
    fn get_stream(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Response, ModelError> {
        let (send, receive) = channel();
        *self.stream.lock().expect("stream") = Some(send);
        self.send("endpoint", &self.endpoint);
        let body = Box::new(BufReader::new(Pipe(receive, Vec::new())));
        Ok(Response { status: 200, headers: vec![("content-type".into(), "text/event-stream".into())], body })
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        self.posted.lock().expect("posted").push((url.into(), headers.to_vec()));
        let message: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        if let Some(id) = message.get("id") {
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
        Ok(Response { status: 202, headers: Vec::new(), body: Box::new(std::io::Cursor::new(Vec::new())) })
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
    let error = McpServer::open(&spec(), &http).expect_err("refused");
    assert!(error.contains("another site"), "{error}");
}

#[test]
fn a_request_after_the_stream_ended_fails_instead_of_waiting() {
    let legacy = Legacy::new("/messages");
    let http: Arc<dyn Http> = legacy.clone();
    let server = McpServer::open(&spec(), &http).expect("connected");
    legacy.stream.lock().expect("stream").take();
    let started = std::time::Instant::now();
    let error = server.initialize_stopped(&std::sync::atomic::AtomicBool::new(false)).expect_err("closed");
    assert!(error.contains("closed its stream"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
fn a_cancelled_run_stops_waiting_for_the_endpoint() {
    struct Silent(Mutex<Option<Sender<Vec<u8>>>>);
    impl Http for Silent {
        fn get_stream(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _stop: &std::sync::atomic::AtomicBool,
        ) -> Result<Response, ModelError> {
            let (send, receive) = channel();
            *self.0.lock().expect("stream") = Some(send);
            Ok(Response { status: 200, headers: Vec::new(), body: Box::new(BufReader::new(Pipe(receive, Vec::new()))) })
        }
        fn post(&self, _: &str, _: &[(String, String)], _: &str) -> Result<Response, String> {
            Err("unused".into())
        }
    }
    let silent = Arc::new(Silent(Mutex::default()));
    let http: Arc<dyn Http> = silent.clone();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let started = std::time::Instant::now();
    let error = McpServer::open_stopped(&spec(), &http, &stop).expect_err("gave up");
    assert!(error.contains("in time"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    drop(silent.0.lock().expect("stream").take());
}
