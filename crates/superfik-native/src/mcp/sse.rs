//! An MCP server over the older HTTP+SSE transport (`"type": "sse"`): a
//! GET opens an event stream whose `endpoint` event names where to POST
//! messages; answers come back on the stream as `message` events, matched
//! to their request by id. The endpoint must be on the server's own site,
//! so its headers (a token) never go elsewhere.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::Transport;
use crate::model::http::{Http, Response, read_line_stopped};
use crate::model::sse::SseDecoder;

type Waiting = Arc<Mutex<HashMap<String, Sender<Value>>>>;

pub struct SseTransport {
    name: String,
    endpoint: String,
    headers: Vec<(String, String)>,
    http: Arc<dyn Http>,
    waiting: Waiting,
    /// Set when the stream ends, and when this transport is dropped (which
    /// ends the stream).
    closed: Arc<AtomicBool>,
}

impl std::fmt::Debug for SseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SseTransport").field("name", &self.name).field("endpoint", &self.endpoint).finish()
    }
}

impl Drop for SseTransport {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

impl SseTransport {
    pub fn connect(
        name: &str,
        url: &str,
        headers: &[(String, String)],
        http: Arc<dyn Http>,
        stop: &AtomicBool,
    ) -> Result<Self, String> {
        let mut asked = headers.to_vec();
        asked.push(("accept".into(), "text/event-stream".into()));
        let mut stream = http.get_stream(url, &asked, stop).map_err(|e| format!("{name}: {e}"))?;
        if !(200..300).contains(&stream.status) {
            return Err(format!("{name} answered HTTP {}", stream.status));
        }
        let mut decoder = SseDecoder::new();
        // The wait for the endpoint ends with the run, or after ENDPOINT_WAIT.
        let given_up = AtomicBool::new(false);
        let found = AtomicBool::new(false);
        let endpoint = std::thread::scope(|scope| {
            scope.spawn(|| {
                let started = std::time::Instant::now();
                while !found.load(Ordering::Relaxed) {
                    if stop.load(Ordering::Relaxed) || started.elapsed() > ENDPOINT_WAIT {
                        given_up.store(true, Ordering::Relaxed);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
            });
            let endpoint = endpoint_of(name, url, &mut stream, &mut decoder, &given_up);
            found.store(true, Ordering::Relaxed);
            endpoint
        })?;
        let transport = Self {
            name: name.to_owned(),
            endpoint,
            headers: headers.to_vec(),
            http,
            waiting: Arc::default(),
            closed: Arc::default(),
        };
        transport.listen(stream, decoder);
        Ok(transport)
    }

    /// Reads the stream on its own thread: answers go to their requests,
    /// the server's pings are answered.
    fn listen(&self, mut stream: Response, mut decoder: SseDecoder) {
        let (waiting, closed) = (Arc::clone(&self.waiting), Arc::clone(&self.closed));
        let poster = self.poster();
        std::thread::spawn(move || {
            loop {
                let mut line = String::new();
                match read_line_stopped(stream.body.as_mut(), &mut line, &closed) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n'])) else { continue };
                let Ok(message) = serde_json::from_str::<Value>(&event.data) else { continue };
                match (message.get("id"), message.get("method").and_then(Value::as_str)) {
                    (Some(id), None) => {
                        let sender = waiting.lock().ok().and_then(|mut waiting| waiting.remove(&id.to_string()));
                        if let Some(sender) = sender {
                            let _ = sender.send(message);
                        }
                    }
                    (Some(id), Some(method)) => {
                        let _ = poster.post(&client_answer(id, method), &closed);
                    }
                    _ => {}
                }
            }
            closed.store(true, Ordering::Relaxed);
            if let Ok(mut waiting) = waiting.lock() {
                waiting.clear();
            }
        });
    }

    fn poster(&self) -> Poster {
        Poster { endpoint: self.endpoint.clone(), headers: self.headers.clone(), http: Arc::clone(&self.http) }
    }
}

struct Poster {
    endpoint: String,
    headers: Vec<(String, String)>,
    http: Arc<dyn Http>,
}

impl Poster {
    fn post(&self, message: &Value, stop: &AtomicBool) -> Result<(), String> {
        let response = self
            .http
            .post_stopped(&self.endpoint, &self.headers, &message.to_string(), stop)
            .map_err(|e| e.to_string())?;
        if (200..300).contains(&response.status) { Ok(()) } else { Err(format!("HTTP {}", response.status)) }
    }
}

impl Transport for SseTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.request_stopped(message, &AtomicBool::new(false))
    }

    fn request_stopped(&self, message: &Value, stop: &AtomicBool) -> Result<Value, String> {
        let id = message.get("id").map(Value::to_string).unwrap_or_default();
        let (send, receive): (Sender<Value>, Receiver<Value>) = channel();
        self.waiting.lock().map_err(|_| "MCP waiters unavailable")?.insert(id.clone(), send);
        if let Err(error) = self.poster().post(message, stop) {
            let _ = self.waiting.lock().map(|mut waiting| waiting.remove(&id));
            return Err(format!("{}: {error}", self.name));
        }
        loop {
            // A stream already ended never answers (its waiters were dropped
            // before this one came).
            if stop.load(Ordering::Relaxed) || self.closed.load(Ordering::Relaxed) {
                if let Ok(reply) = receive.try_recv() {
                    return Ok(reply);
                }
                let _ = self.waiting.lock().map(|mut waiting| waiting.remove(&id));
                return Err(if stop.load(Ordering::Relaxed) {
                    "cancelled".into()
                } else {
                    format!("{} closed its stream", self.name)
                });
            }
            match receive.recv_timeout(Duration::from_millis(25)) {
                Ok(reply) => return Ok(reply),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(format!("{} closed its stream", self.name)),
            }
        }
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        self.notify_stopped(message, &AtomicBool::new(false))
    }

    fn notify_stopped(&self, message: &Value, stop: &AtomicBool) -> Result<(), String> {
        self.poster().post(message, stop).map_err(|error| format!("{}: {error}", self.name))
    }
}

/// How long a server may take to name its endpoint.
const ENDPOINT_WAIT: Duration = Duration::from_secs(30);

/// The endpoint the stream's `endpoint` event names, read until `given_up`.
fn endpoint_of(
    name: &str,
    url: &str,
    stream: &mut Response,
    decoder: &mut SseDecoder,
    given_up: &AtomicBool,
) -> Result<String, String> {
    loop {
        let mut line = String::new();
        match read_line_stopped(stream.body.as_mut(), &mut line, given_up) {
            Err(crate::model::ModelError::Cancelled) => {
                return Err(format!("{name} did not name its endpoint in time"));
            }
            Err(error) => return Err(error.to_string()),
            Ok(0) => return Err(format!("{name} closed its stream before naming its endpoint")),
            Ok(_) => {}
        }
        if let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n']))
            && event.event == "endpoint"
        {
            return same_site(url, event.data.trim())
                .ok_or_else(|| format!("{name} named an endpoint on another site ({}); refused", event.data.trim()));
        }
    }
}

/// `endpoint` made absolute against `url`, when it is on the same site.
fn same_site(url: &str, endpoint: &str) -> Option<String> {
    let base = url::Url::parse(url).ok()?;
    let joined = base.join(endpoint).ok()?;
    (joined.origin() == base.origin()).then(|| joined.to_string())
}

/// What the client answers a server's own request with.
pub(super) fn client_answer(id: &Value, method: &str) -> Value {
    if method == "ping" {
        json!({ "jsonrpc": "2.0", "id": id, "result": {} })
    } else {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Client method not supported" } })
    }
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
