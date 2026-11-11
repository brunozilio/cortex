use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{Listener, PROTOCOL_VERSION, Transport};
use crate::model::http::{Http, Response};

pub struct HttpTransport {
    shared: Arc<Shared>,
    listener: Option<Listener>,
    closed: Arc<AtomicBool>,
    listening: AtomicBool,
}

pub(super) struct Shared {
    pub(super) name: String,
    url: String,
    headers: Vec<(String, String)>,
    http: Arc<dyn Http>,
    session: Mutex<Option<String>>,
}

impl std::fmt::Debug for HttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shared = &self.shared;
        f.debug_struct("HttpTransport").field("name", &shared.name).field("url", &shared.url).finish_non_exhaustive()
    }
}

impl Drop for HttpTransport {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

impl Shared {
    fn headers(&self, accept: &str) -> Vec<(String, String)> {
        let mut headers = self.headers.clone();
        headers.push(("accept".to_owned(), accept.to_owned()));
        headers.push(("mcp-protocol-version".to_owned(), PROTOCOL_VERSION.to_owned()));
        if let Some(session) = self.session.lock().ok().and_then(|session| session.clone()) {
            headers.push(("mcp-session-id".to_owned(), session));
        }
        headers
    }

    pub(super) fn post(&self, message: &Value, stop: &AtomicBool) -> Result<Response, String> {
        let headers = self.headers("application/json, text/event-stream");
        let response =
            self.http.post_stopped(&self.url, &headers, &message.to_string(), stop).map_err(|e| e.to_string())?;
        let status = response.status;
        if let Some(session) = response.header("mcp-session-id")
            && let Ok(mut kept) = self.session.lock()
        {
            *kept = Some(session);
        }
        if status == 401 {
            return Err(format!("{} answered HTTP 401: sign in with /mcp login {}", self.name, self.name));
        }
        if !(200..300).contains(&status) {
            return Err(format!("{} answered HTTP {status}", self.name));
        }
        Ok(response)
    }

    pub(super) fn stream(&self, stop: &AtomicBool) -> Result<Option<Response>, String> {
        let response = self.http.get_stream(&self.url, &self.headers("text/event-stream"), stop);
        let response = response.map_err(|e| e.to_string())?;
        match response.status {
            405 => Ok(None),
            status if (200..300).contains(&status) => Ok(Some(response)),
            status => Err(format!("{} answered HTTP {status} to its stream", self.name)),
        }
    }
}

impl HttpTransport {
    pub fn new(name: &str, url: &str, headers: &[(String, String)], http: Arc<dyn Http>) -> Self {
        Self::listening(name, url, headers, http, None)
    }

    pub fn listening(
        name: &str,
        url: &str,
        headers: &[(String, String)],
        http: Arc<dyn Http>,
        listener: Option<Listener>,
    ) -> Self {
        let shared = Shared {
            name: name.to_owned(),
            url: url.to_owned(),
            headers: headers.to_vec(),
            http,
            session: Mutex::new(None),
        };
        Self { shared: Arc::new(shared), listener, closed: Arc::default(), listening: AtomicBool::new(false) }
    }
}

impl Transport for HttpTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.request_stopped(message, &AtomicBool::new(false))
    }

    fn request_stopped(&self, message: &Value, stop: &AtomicBool) -> Result<Value, String> {
        let mut response = self.shared.post(message, stop)?;
        let mut text = String::new();
        let mut decoder = crate::model::sse::SseDecoder::new();
        loop {
            let mut line = String::new();
            let read = crate::model::http::read_line_stopped(response.body.as_mut(), &mut line, stop)
                .map_err(|e| e.to_string())?;
            text.push_str(&line);
            if let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n']))
                && let Ok(reply) = serde_json::from_str::<Value>(&event.data)
            {
                match (reply.get("id"), reply.get("method").and_then(Value::as_str)) {
                    (id, None) if id == message.get("id") => return Ok(reply),
                    (Some(id), Some(method)) => {
                        self.notify_stopped(&super::sse::client_answer(id, method), stop)?;
                    }
                    (None, Some(method)) => {
                        if let Some(listener) = &self.listener {
                            listener(method, &reply["params"]);
                        }
                    }
                    _ => {}
                }
            }
            if text.len() > 16 * 1024 * 1024 {
                return Err("MCP response exceeds size limit".into());
            }
            if let Some(reply) = parse(&text).filter(|r| r.get("id") == message.get("id") && r.get("method").is_none())
            {
                return Ok(reply);
            }
            if read == 0 {
                return Err(format!("{} sent an unreadable answer", self.shared.name));
            }
        }
    }

    fn notify_stopped(&self, message: &Value, stop: &AtomicBool) -> Result<(), String> {
        self.shared.post(message, stop).map(|_| ())
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        self.notify_stopped(message, &AtomicBool::new(false))
    }

    fn listen(&self) {
        let Some(listener) = &self.listener else { return };
        if self.listening.swap(true, Ordering::Relaxed) {
            return;
        }
        let (shared, listener, closed) = (Arc::clone(&self.shared), Arc::clone(listener), Arc::clone(&self.closed));
        let spawned = std::thread::Builder::new()
            .name(format!("{brand_command}-mcp-{}-stream", shared.name, brand_command = brand::COMMAND))
            .spawn(move || super::http_listen::run(&shared, &listener, &closed));
        if let Err(error) = spawned {
            tracing::warn!(%error, server = self.shared.name, "an MCP server's stream could not be opened");
        }
    }
}

pub fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(text.trim()).ok().or_else(|| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|data| serde_json::from_str::<Value>(data.trim()).ok().filter(|v| v.get("id").is_some()))
    })
}
