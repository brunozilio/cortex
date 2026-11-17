use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use platform::sync::Locked;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::Listener;
use super::http_listen::{FIRST_WAIT, MOST_WAIT, pause};
use crate::model::http::{Http, Response};
use crate::model::sse::SseDecoder;
use crate::model::{CancellationToken, ModelError};

const ENDPOINT_WAIT: Duration = Duration::from_secs(30);

pub(super) type Waiting = Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>;

pub(super) struct Link {
    pub(super) name: String,
    pub(super) url: String,
    pub(super) endpoint: Mutex<String>,
    pub(super) headers: Vec<(String, String)>,
    pub(super) http: Arc<dyn Http>,
}

impl Link {
    pub(super) async fn post(&self, message: &Value, cancel: &CancellationToken) -> Result<(), String> {
        let endpoint = self.endpoint.locked().clone();
        let body = message.to_string();
        let response =
            self.http.post_cancellable(&endpoint, &self.headers, &body, cancel).await.map_err(|e| e.to_string())?;
        if (200..300).contains(&response.status) { Ok(()) } else { Err(format!("HTTP {}", response.status)) }
    }
}

pub(super) struct Opened {
    pub(super) stream: Response,
    pub(super) decoder: SseDecoder,
    pub(super) endpoint: String,
}

pub(super) async fn open(
    name: &str,
    url: &str,
    headers: &[(String, String)],
    http: &Arc<dyn Http>,
    cancel: &CancellationToken,
) -> Result<Opened, String> {
    let mut asked = headers.to_vec();
    asked.push(("accept".into(), "text/event-stream".into()));
    let mut stream = http.get_stream(url, &asked, cancel).await.map_err(|e| format!("{name}: {e}"))?;
    if !(200..300).contains(&stream.status) {
        return Err(format!("{name} answered HTTP {}", stream.status));
    }
    let mut decoder = SseDecoder::new();
    let late = || format!("{name} did not name its endpoint in time");
    let endpoint = tokio::select! {
        biased;
        () = cancel.cancelled() => Err(late()),
        () = tokio::time::sleep(ENDPOINT_WAIT) => Err(late()),
        named = endpoint_of(name, url, &mut stream, &mut decoder, cancel) => named,
    }?;
    Ok(Opened { stream, decoder, endpoint })
}

async fn endpoint_of(
    name: &str,
    url: &str,
    stream: &mut Response,
    decoder: &mut SseDecoder,
    cancel: &CancellationToken,
) -> Result<String, String> {
    loop {
        let mut line = String::new();
        match stream.body.read_line(&mut line, cancel).await {
            Err(ModelError::Cancelled) => return Err(format!("{name} did not name its endpoint in time")),
            Err(error) => return Err(error.to_string()),
            Ok(0) => return Err(format!("{name} closed its stream before naming its endpoint")),
            Ok(_) => {}
        }
        if let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n'])).map_err(|error| error.to_string())?
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
    pub(super) closed: CancellationToken,
    pub(super) down: Arc<AtomicBool>,
    pub(super) greeting: Arc<Mutex<Option<Value>>>,
    pub(super) listener: Option<Listener>,
}

impl Listening {
    pub(super) async fn run(self, stream: Response, decoder: SseDecoder) {
        let mut open_stream = Some((stream, decoder, None));
        let mut wait = FIRST_WAIT;
        while !self.closed.is_cancelled() {
            if let Some((stream, decoder, greeted)) = open_stream.take() {
                if self.read(stream, decoder, greeted).await {
                    wait = FIRST_WAIT;
                }
                self.went_down();
                continue;
            }
            if !pause(wait, &self.closed).await {
                break;
            }
            wait = (wait * 2).min(MOST_WAIT);
            open_stream = self.reopened().await;
        }
        self.went_down();
    }

    fn went_down(&self) {
        self.down.store(true, Ordering::SeqCst);
        self.waiting.locked().clear();
    }

    async fn reopened(&self) -> Option<(Response, SseDecoder, Option<String>)> {
        let link = &self.link;
        let opened = match open(&link.name, &link.url, &link.headers, &link.http, &self.closed).await {
            Ok(opened) => opened,
            Err(error) => {
                tracing::warn!(%error, server = link.name, "an MCP server's stream is down");
                return None;
            }
        };
        *link.endpoint.locked() = opened.endpoint;
        match self.greet().await {
            Ok(greeted) => Some((opened.stream, opened.decoder, greeted)),
            Err(error) => {
                tracing::warn!(%error, server = link.name, "an MCP server's stream is down");
                None
            }
        }
    }

    async fn greet(&self) -> Result<Option<String>, String> {
        let greeting = self.greeting.locked().clone();
        let Some(greeting) = greeting else {
            self.down.store(false, Ordering::SeqCst);
            return Ok(None);
        };
        self.link.post(&greeting, &self.closed).await?;
        Ok(greeting.get("id").map(Value::to_string))
    }

    async fn read(&self, mut stream: Response, mut decoder: SseDecoder, mut greeted: Option<String>) -> bool {
        let mut heard = false;
        loop {
            let mut line = String::new();
            match stream.body.read_line(&mut line, &self.closed).await {
                Ok(0) | Err(_) => return heard,
                Ok(_) => {}
            }
            let event = match decoder.line(line.trim_end_matches(['\r', '\n'])) {
                Ok(Some(event)) => event,
                Ok(None) => continue,
                Err(_) => return heard,
            };
            let Ok(message) = serde_json::from_str::<Value>(&event.data) else { continue };
            heard = true;
            match (message.get("id"), message.get("method").and_then(Value::as_str)) {
                (Some(id), None) if greeted.as_deref() == Some(id.to_string().as_str()) => {
                    greeted = None;
                    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
                    if self.link.post(&initialized, &self.closed).await.is_err() {
                        return heard;
                    }
                    self.down.store(false, Ordering::SeqCst);
                }
                (Some(id), None) => {
                    let sender = self.waiting.locked().remove(&id.to_string());
                    if let Some(sender) = sender {
                        let _ = sender.send(message);
                    }
                }
                (Some(id), Some(method)) => {
                    let _ = self.link.post(&super::sse::client_answer(id, method), &self.closed).await;
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
