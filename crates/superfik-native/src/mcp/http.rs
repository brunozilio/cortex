//! An MCP server over streamable HTTP: each message POSTed, the answer as
//! JSON or as the first message of an event stream. A session id the server
//! hands out is sent back with every later message.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{PROTOCOL_VERSION, Transport};
use crate::model::http::Http;

pub struct HttpTransport {
    name: String,
    url: String,
    headers: Vec<(String, String)>,
    http: Arc<dyn Http>,
    session: Mutex<Option<String>>,
}

impl std::fmt::Debug for HttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransport").field("name", &self.name).field("url", &self.url).finish_non_exhaustive()
    }
}

impl HttpTransport {
    pub fn new(name: &str, url: &str, headers: &[(String, String)], http: Arc<dyn Http>) -> Self {
        Self { name: name.to_owned(), url: url.to_owned(), headers: headers.to_vec(), http, session: Mutex::new(None) }
    }

    fn response(
        &self,
        message: &Value,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::model::http::Response, String> {
        let mut headers = self.headers.clone();
        headers.push(("accept".to_owned(), "application/json, text/event-stream".to_owned()));
        headers.push(("mcp-protocol-version".to_owned(), PROTOCOL_VERSION.to_owned()));
        if let Some(session) = self.session.lock().ok().and_then(|session| session.clone()) {
            headers.push(("mcp-session-id".to_owned(), session));
        }
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
}

impl Transport for HttpTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.request_stopped(message, &std::sync::atomic::AtomicBool::new(false))
    }

    fn request_stopped(&self, message: &Value, stop: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        let mut response = self.response(message, stop)?;
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
                if reply.get("id") == message.get("id") && reply.get("method").is_none() {
                    return Ok(reply);
                }
                if let (Some(id), Some(method)) = (reply.get("id"), reply.get("method").and_then(Value::as_str)) {
                    self.notify_stopped(&super::sse::client_answer(id, method), stop)?;
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
                return Err(format!("{} sent an unreadable answer", self.name));
            }
        }
    }
    fn notify_stopped(&self, message: &Value, stop: &std::sync::atomic::AtomicBool) -> Result<(), String> {
        self.response(message, stop).map(|_| ())
    }
    fn notify(&self, message: &Value) -> Result<(), String> {
        self.notify_stopped(message, &std::sync::atomic::AtomicBool::new(false))
    }
}

pub fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(text.trim()).ok().or_else(|| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|data| serde_json::from_str::<Value>(data.trim()).ok().filter(|v| v.get("id").is_some()))
    })
}
