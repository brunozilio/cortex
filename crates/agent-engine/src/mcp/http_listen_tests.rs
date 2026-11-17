use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::mpsc::{Sender, channel};

use crate::mcp::{Listener, McpServer, ServerSpec};
use crate::model::http::{Body, Http, Response};
use crate::model::{BoxFuture, CancellationToken, ModelError};

type Chunk = Result<Vec<u8>, String>;

struct Streamable {
    get_status: u16,
    streams: Mutex<Vec<Sender<Chunk>>>,
    opened: Mutex<Vec<Option<String>>>,
}

fn event(message: &Value) -> String {
    format!("event: message\ndata: {message}\n\n")
}

impl Streamable {
    fn new(get_status: u16) -> Arc<Self> {
        Arc::new(Self { get_status, streams: Mutex::default(), opened: Mutex::default() })
    }

    fn say(&self, content: &str) {
        let mut streams = self.streams.lock().expect("streams");
        let latest = streams.pop();
        streams.clear();
        if let Some(stream) = latest {
            let note =
                json!({ "jsonrpc": "2.0", "method": "notifications/claude/channel", "params": { "content": content } });
            let _ = stream.try_send(Ok(event(&note).into_bytes()));
            streams.push(stream);
        }
    }

    fn hang_up(&self) {
        self.streams.lock().expect("streams").clear();
    }

    fn gets(&self) -> usize {
        self.opened.lock().expect("opened").len()
    }

    fn is_streaming(&self) -> bool {
        self.streams.lock().expect("streams").iter().any(|stream| !stream.is_closed())
    }

    fn open_stream(&self, headers: &[(String, String)]) -> Result<Response, ModelError> {
        let session = headers.iter().find(|(name, _)| name == "mcp-session-id").map(|(_, value)| value.clone());
        self.opened.lock().expect("opened").push(session);
        if self.get_status != 200 {
            return Ok(Response { status: self.get_status, headers: Vec::new(), body: Body::from(Vec::new()) });
        }
        let (send, receive) = channel(64);
        self.streams.lock().expect("streams").push(send);
        Ok(Response { status: 200, headers: Vec::new(), body: Body::chunks(receive) })
    }
}

impl Http for Streamable {
    fn get_stream<'a>(
        &'a self,
        _url: &'a str,
        headers: &'a [(String, String)],
        _cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Response, ModelError>> {
        Box::pin(std::future::ready(self.open_stream(headers)))
    }

    fn post(&self, _url: &str, _headers: &[(String, String)], body: &str) -> Result<Response, String> {
        let message: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        let session = vec![("mcp-session-id".to_owned(), "s-1".to_owned())];
        let Some(id) = message.get("id") else {
            return Ok(Response { status: 202, headers: session, body: Body::from(Vec::new()) });
        };
        let result = match message["method"].as_str() {
            Some("initialize") => json!({ "capabilities": { "experimental": { "claude/channel": {} } } }),
            _ => json!({ "content": [{ "type": "text", "text": "sent" }] }),
        };
        let progress = json!({ "jsonrpc": "2.0", "method": "notifications/claude/channel", "params": { "content": "while answering" } });
        let text = event(&progress) + &event(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        Ok(Response { status: 200, headers: session, body: Body::from(text) })
    }
}

fn spec() -> ServerSpec {
    ServerSpec::Http { name: "chat".into(), url: "https://chat.example/mcp".into(), headers: Vec::new() }
}

fn listened(server: &Arc<Streamable>) -> (McpServer, Arc<Mutex<Vec<String>>>) {
    let heard: Arc<Mutex<Vec<String>>> = Arc::default();
    let kept = Arc::clone(&heard);
    let listener: Listener = Arc::new(move |_method, params| {
        kept.lock().expect("heard").push(params["content"].as_str().unwrap_or_default().to_owned());
    });
    let http: Arc<dyn Http> = Arc::clone(server) as Arc<dyn Http>;
    let opened = McpServer::open_listening(&spec(), &http, &platform::Stop::new(), Some(listener)).expect("open");
    opened.initialize_stopped(&platform::Stop::new()).expect("initialize");
    (opened, heard)
}

fn until(what: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(120);
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
    let answer = opened.ask("tools/call", json!({ "name": "reply" }), &platform::Stop::new()).expect("answer");
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
fn closing_a_listened_server_ends_its_stream_at_once() {
    let server = Streamable::new(200);
    let (opened, _heard) = listened(&server);
    assert!(until(|| server.is_streaming()), "the stream is open");
    let closed = Instant::now();
    opened.close();
    assert!(until(|| !server.is_streaming()), "the stream was let go");
    let took = closed.elapsed();
    eprintln!("closing a listened HTTP server let its stream go after {took:?}");
    assert!(took < Duration::from_secs(1), "{took:?}");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(server.gets(), 1, "a closed server's stream is not opened again");
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
    opened.initialize_stopped(&platform::Stop::new()).expect("initialize");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(server.gets(), 0);
}
