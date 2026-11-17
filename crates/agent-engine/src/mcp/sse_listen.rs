use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use platform::sync::Locked;
use serde_json::{Value, json};

use super::Listener;
use super::http_listen::{FIRST_WAIT, MOST_WAIT, pause};
use crate::model::http::{Http, Response, read_line_stopped};
use crate::model::sse::SseDecoder;

const ENDPOINT_WAIT: Duration = Duration::from_secs(30);
const STOP_CHECK: Duration = Duration::from_millis(25);

pub(super) type Waiting = Arc<Mutex<HashMap<String, Sender<Value>>>>;

pub(super) struct Link {
    pub(super) name: String,
    pub(super) url: String,
    pub(super) endpoint: Mutex<String>,
    pub(super) headers: Vec<(String, String)>,
    pub(super) http: Arc<dyn Http>,
}

impl Link {
    pub(super) fn post(&self, message: &Value, stop: &AtomicBool) -> Result<(), String> {
        let endpoint = self.endpoint.locked().clone();
        let response =
            self.http.post_stopped(&endpoint, &self.headers, &message.to_string(), stop).map_err(|e| e.to_string())?;
        if (200..300).contains(&response.status) { Ok(()) } else { Err(format!("HTTP {}", response.status)) }
    }
}

pub(super) struct Opened {
    pub(super) stream: Response,
    pub(super) decoder: SseDecoder,
    pub(super) endpoint: String,
}

pub(super) fn open(
    name: &str,
    url: &str,
    headers: &[(String, String)],
    http: &Arc<dyn Http>,
    stop: &AtomicBool,
) -> Result<Opened, String> {
    let mut asked = headers.to_vec();
    asked.push(("accept".into(), "text/event-stream".into()));
    let mut stream = http.get_stream(url, &asked, stop).map_err(|e| format!("{name}: {e}"))?;
    if !(200..300).contains(&stream.status) {
        return Err(format!("{name} answered HTTP {}", stream.status));
    }
    let mut decoder = SseDecoder::new();
    let given_up = AtomicBool::new(false);
    let (found, waiting) = std::sync::mpsc::channel::<()>();
    let endpoint = std::thread::scope(|scope| {
        let given_up = &given_up;
        scope.spawn(move || {
            let deadline = std::time::Instant::now() + ENDPOINT_WAIT;
            loop {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                match waiting.recv_timeout(left.min(STOP_CHECK)) {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                        if !stop.load(Ordering::Relaxed) && !left.is_zero() => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        given_up.store(true, Ordering::Relaxed);
                        return;
                    }
                    _ => return,
                }
            }
        });
        let endpoint = endpoint_of(name, url, &mut stream, &mut decoder, given_up);
        drop(found);
        endpoint
    })?;
    Ok(Opened { stream, decoder, endpoint })
}

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

fn same_site(url: &str, endpoint: &str) -> Option<String> {
    let base = url::Url::parse(url).ok()?;
    let joined = base.join(endpoint).ok()?;
    (joined.origin() == base.origin()).then(|| joined.to_string())
}

pub(super) struct Listening {
    pub(super) link: Arc<Link>,
    pub(super) waiting: Waiting,
    pub(super) closed: Arc<AtomicBool>,
    pub(super) down: Arc<AtomicBool>,
    pub(super) greeting: Arc<Mutex<Option<Value>>>,
    pub(super) listener: Option<Listener>,
}

impl Listening {
    pub(super) fn run(self, stream: Response, decoder: SseDecoder) {
        let mut open_stream = Some((stream, decoder, None));
        let mut wait = FIRST_WAIT;
        while !self.closed.load(Ordering::Relaxed) {
            if let Some((stream, decoder, greeted)) = open_stream.take() {
                if self.read(stream, decoder, greeted) {
                    wait = FIRST_WAIT;
                }
                self.down.store(true, Ordering::Relaxed);
                self.waiting.locked().clear();
                continue;
            }
            pause(wait, &self.closed);
            wait = (wait * 2).min(MOST_WAIT);
            open_stream = self.reopened();
        }
        self.down.store(true, Ordering::Relaxed);
        self.waiting.locked().clear();
    }

    fn reopened(&self) -> Option<(Response, SseDecoder, Option<String>)> {
        let link = &self.link;
        let opened = open(&link.name, &link.url, &link.headers, &link.http, &self.closed).and_then(|opened| {
            *link.endpoint.locked() = opened.endpoint;
            let greeted = self.greet()?;
            Ok((opened.stream, opened.decoder, greeted))
        });
        opened.inspect_err(|error| tracing::warn!(%error, server = link.name, "an MCP server's stream is down")).ok()
    }

    fn greet(&self) -> Result<Option<String>, String> {
        let Some(greeting) = self.greeting.locked().clone() else {
            self.down.store(false, Ordering::Relaxed);
            return Ok(None);
        };
        self.link.post(&greeting, &self.closed)?;
        Ok(greeting.get("id").map(Value::to_string))
    }

    fn read(&self, mut stream: Response, mut decoder: SseDecoder, mut greeted: Option<String>) -> bool {
        let mut heard = false;
        loop {
            let mut line = String::new();
            match read_line_stopped(stream.body.as_mut(), &mut line, &self.closed) {
                Ok(0) | Err(_) => return heard,
                Ok(_) => {}
            }
            let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n'])) else { continue };
            let Ok(message) = serde_json::from_str::<Value>(&event.data) else { continue };
            heard = true;
            match (message.get("id"), message.get("method").and_then(Value::as_str)) {
                (Some(id), None) if greeted.as_deref() == Some(id.to_string().as_str()) => {
                    greeted = None;
                    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
                    if self.link.post(&initialized, &self.closed).is_err() {
                        return heard;
                    }
                    self.down.store(false, Ordering::Relaxed);
                }
                (Some(id), None) => {
                    let sender = self.waiting.locked().remove(&id.to_string());
                    if let Some(sender) = sender {
                        let _ = sender.send(message);
                    }
                }
                (Some(id), Some(method)) => {
                    let _ = self.link.post(&super::sse::client_answer(id, method), &self.closed);
                }
                (None, Some(method)) => {
                    if let Some(listener) = &self.listener {
                        listener(method, &message["params"]);
                    }
                }
                (None, None) => {}
            }
        }
    }
}
