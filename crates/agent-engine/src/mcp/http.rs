use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{Listener, MESSAGE_AT_MOST, McpError, PROTOCOL_VERSION, Transport};
use crate::model::CancellationToken;
use crate::model::http::{Http, Response};

pub struct HttpTransport {
    shared: Arc<Shared>,
    listener: Option<Listener>,
    closed: CancellationToken,
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
        self.closed.cancel();
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

    pub(super) async fn post(&self, message: &Value, cancel: &CancellationToken) -> Result<Response, McpError> {
        let headers = self.headers("application/json, text/event-stream");
        let body = message.to_string();
        let response =
            self.http.post_cancellable(&self.url, &headers, &body, cancel).await.map_err(|e| e.to_string())?;
        let status = response.status;
        if let Some(session) = response.header("mcp-session-id")
            && let Ok(mut kept) = self.session.lock()
        {
            *kept = Some(session);
        }
        if status == 401 {
            return Err(McpError::SignInNeeded { server: self.name.clone() });
        }
        if !(200..300).contains(&status) {
            return Err(format!("{} answered HTTP {status}", self.name).into());
        }
        Ok(response)
    }

    pub(super) async fn stream(&self, cancel: &CancellationToken) -> Result<Option<Response>, McpError> {
        let headers = self.headers("text/event-stream");
        let response = self.http.get_stream(&self.url, &headers, cancel).await.map_err(|e| e.to_string())?;
        match response.status {
            405 => Ok(None),
            status if (200..300).contains(&status) => Ok(Some(response)),
            status => Err(format!("{} answered HTTP {status} to its stream", self.name).into()),
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
        Self { shared: Arc::new(shared), listener, closed: CancellationToken::new(), listening: AtomicBool::new(false) }
    }

    async fn requested(&self, message: &Value, cancel: &CancellationToken) -> Result<Value, McpError> {
        let mut response = self.shared.post(message, cancel).await?;
        let mut text = String::new();
        let mut decoder = crate::model::sse::SseDecoder::new();
        loop {
            let mut line = String::new();
            let read = response.body.read_line(&mut line, cancel).await.map_err(|e| e.to_string())?;
            text.push_str(&line);
            if let Some(event) = decoder.line(line.trim_end_matches(['\r', '\n'])).map_err(|e| e.to_string())?
                && let Ok(reply) = serde_json::from_str::<Value>(&event.data)
            {
                match (reply.get("id"), reply.get("method").and_then(Value::as_str)) {
                    (id, None) if id == message.get("id") => return Ok(reply),
                    (Some(id), Some(method)) => {
                        self.shared.post(&super::sse::client_answer(id, method), cancel).await?;
                    }
                    (None, Some(method)) => {
                        if let Some(listener) = &self.listener {
                            listener(method, &reply["params"]);
                        }
                    }
                    _ => {}
                }
            }
            if text.len() > MESSAGE_AT_MOST {
                return Err("MCP response exceeds size limit".into());
            }
            let answers = |reply: &Value| reply.get("id") == message.get("id") && reply.get("method").is_none();
            if let Some(reply) = on_one_line(&line).filter(answers) {
                return Ok(reply);
            }
            if read == 0 {
                let unreadable = || format!("{} sent an unreadable answer", self.shared.name);
                return parse(&text).filter(answers).ok_or_else(|| unreadable().into());
            }
        }
    }
}

impl Transport for HttpTransport {
    fn request(&self, message: &Value) -> Result<Value, McpError> {
        self.request_stopped(message, &platform::Stop::new())
    }

    fn request_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<Value, McpError> {
        platform::runtime::block_on(self.requested(message, stop.token()))
            .unwrap_or_else(|error| Err(error.to_string().into()))
    }

    fn notify_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<(), McpError> {
        platform::runtime::block_on(self.shared.post(message, stop.token()))
            .unwrap_or_else(|error| Err(error.to_string().into()))
            .map(|_| ())
    }

    fn notify(&self, message: &Value) -> Result<(), McpError> {
        self.notify_stopped(message, &platform::Stop::new())
    }

    fn listen(&self) {
        let Some(listener) = &self.listener else { return };
        if self.listening.swap(true, Ordering::Relaxed) {
            return;
        }
        let (shared, listener, closed) = (Arc::clone(&self.shared), Arc::clone(listener), self.closed.clone());
        match platform::runtime::handle() {
            Ok(runtime) => {
                drop(runtime.spawn(async move { super::http_listen::run(&shared, &listener, &closed).await }))
            }
            Err(error) => {
                tracing::warn!(%error, server = self.shared.name, "an MCP server's stream could not be opened")
            }
        }
    }

    fn close(&self) {
        self.closed.cancel();
    }
}

fn on_one_line(line: &str) -> Option<Value> {
    let line = line.trim();
    let message = line.strip_prefix("data:").unwrap_or(line).trim_start();
    message.starts_with('{').then(|| serde_json::from_str(message).ok()).flatten()
}

pub fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(text.trim()).ok().or_else(|| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|data| serde_json::from_str::<Value>(data.trim()).ok().filter(|v| v.get("id").is_some()))
    })
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
