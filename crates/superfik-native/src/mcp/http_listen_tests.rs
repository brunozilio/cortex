use std::io::{BufReader, Cursor, Read};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::mcp::{Listener, McpServer, ServerSpec};
use crate::model::ModelError;
use crate::model::http::{Http, Response};

/// A streamable HTTP server: answers in an event stream that first carries
/// a notification, and keeps GET streams the test writes to.
struct Streamable {
    /// What its GET answers: 200 (a stream) or 405.
    get_status: u16,
    streams: Mutex<Vec<Sender<Vec<u8>>>>,
    /// The session id each GET carried.
    opened: Mutex<Vec<Option<String>>>,
}

struct Pipe(Receiver<Vec<u8>>, Vec<u8>);

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.1.is_empty() {
            match self.0.recv_timeout(Duration::from_millis(25)) {
                Ok(bytes) => self.1 = bytes,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Err(std::io::ErrorKind::TimedOut.into()),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
        let n = buf.len().min(self.1.len());
        buf[..n].copy_from_slice(&self.1[..n]);
        self.1.drain(..n);
        Ok(n)
    }
}

fn event(message: &Value) -> String {
    format!("event: message\ndata: {message}\n\n")
}

impl Streamable {
    fn new(get_status: u16) -> Arc<Self> {
        Arc::new(Self { get_status, streams: Mutex::default(), opened: Mutex::default() })
    }

    /// Says `content` on the latest GET stream, ending the earlier ones.
    fn say(&self, content: &str) {
        let mut streams = self.streams.lock().expect("streams");
        let latest = streams.pop();
        streams.clear();
        if let Some(stream) = latest {
            let note =
                json!({ "jsonrpc": "2.0", "method": "notifications/claude/channel", "params": { "content": content } });
            let _ = stream.send(event(&note).into_bytes());
            streams.push(stream);
        }
    }

    /// Ends every GET stream, as a server restarting would.
    fn hang_up(&self) {
        self.streams.lock().expect("streams").clear();
    }

    fn gets(&self) -> usize {
        self.opened.lock().expect("opened").len()
    }
}

impl Http for Streamable {
    fn get_stream(&self, _url: &str, headers: &[(String, String)], _stop: &AtomicBool) -> Result<Response, ModelError> {
        let session = headers.iter().find(|(name, _)| name == "mcp-session-id").map(|(_, value)| value.clone());
        self.opened.lock().expect("opened").push(session);
        if self.get_status != 200 {
            return Ok(Response {
                status: self.get_status,
                headers: Vec::new(),
                body: Box::new(Cursor::new(Vec::new())),
            });
        }
        let (send, receive) = channel();
        self.streams.lock().expect("streams").push(send);
        let body = Box::new(BufReader::new(Pipe(receive, Vec::new())));
        Ok(Response { status: 200, headers: Vec::new(), body })
    }

    fn post(&self, _url: &str, _headers: &[(String, String)], body: &str) -> Result<Response, String> {
        let message: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        let session = vec![("mcp-session-id".to_owned(), "s-1".to_owned())];
        let Some(id) = message.get("id") else {
            return Ok(Response { status: 202, headers: session, body: Box::new(Cursor::new(Vec::new())) });
        };
        let result = match message["method"].as_str() {
            Some("initialize") => json!({ "capabilities": { "experimental": { "claude/channel": {} } } }),
            _ => json!({ "content": [{ "type": "text", "text": "sent" }] }),
        };
        let progress = json!({ "jsonrpc": "2.0", "method": "notifications/claude/channel", "params": { "content": "while answering" } });
        let text = event(&progress) + &event(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        Ok(Response { status: 200, headers: session, body: Box::new(Cursor::new(text.into_bytes())) })
    }
}

fn spec() -> ServerSpec {
    ServerSpec::Http { name: "chat".into(), url: "https://chat.example/mcp".into(), headers: Vec::new() }
}

/// Opens the server listening, and what its listener heard.
fn listened(server: &Arc<Streamable>) -> (McpServer, Arc<Mutex<Vec<String>>>) {
    let heard: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = Arc::clone(&heard);
    let listener: Listener = Arc::new(move |_method, params| {
        kept.lock().expect("heard").push(params["content"].as_str().unwrap_or_default().to_owned());
    });
    let http: Arc<dyn Http> = Arc::clone(server) as Arc<dyn Http>;
    let opened = McpServer::open_listening(&spec(), &http, &AtomicBool::new(false), Some(listener)).expect("open");
    opened.initialize_stopped(&AtomicBool::new(false)).expect("initialize");
    (opened, heard)
}

fn until(what: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !what() {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

#[test]
fn a_streamable_server_is_heard_on_its_answers_and_on_its_own_stream() {
    let server = Streamable::new(200);
    let (opened, heard) = listened(&server);
    assert!(until(|| server.gets() == 1), "the stream opens once the session is");
    assert_eq!(server.opened.lock().expect("opened")[0].as_deref(), Some("s-1"), "on the session");

    server.say("deploy please");
    assert!(until(|| heard.lock().expect("heard").contains(&"deploy please".to_owned())));
    let answer = opened.ask("tools/call", json!({ "name": "reply" }), &AtomicBool::new(false)).expect("answer");
    assert_eq!(answer["content"][0]["text"], "sent");
    let heard = heard.lock().expect("heard").clone();
    assert_eq!(heard.iter().filter(|said| *said == "while answering").count(), 2, "initialize's and the call's");
}

#[test]
fn a_stream_that_ends_is_opened_again_and_ends_with_the_server() {
    let server = Streamable::new(200);
    let (opened, heard) = listened(&server);
    assert!(until(|| server.gets() == 1));
    server.hang_up();
    assert!(until(|| server.gets() == 2), "opened again");
    server.say("back");
    assert!(until(|| heard.lock().expect("heard").contains(&"back".to_owned())));

    drop(opened);
    server.hang_up();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.gets(), 2, "a dropped server's stream is not opened again");
}

#[test]
fn a_server_without_a_stream_is_asked_once() {
    let server = Streamable::new(405);
    let (_opened, _heard) = listened(&server);
    assert!(until(|| server.gets() == 1));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.gets(), 1);
}

#[test]
fn an_unlistened_server_opens_no_stream() {
    let server = Streamable::new(200);
    let http: Arc<dyn Http> = Arc::clone(&server) as Arc<dyn Http>;
    let opened = McpServer::open(&spec(), &http).expect("open");
    opened.initialize_stopped(&AtomicBool::new(false)).expect("initialize");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(server.gets(), 0);
}
