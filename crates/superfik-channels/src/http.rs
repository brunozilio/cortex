//! HTTP behind a trait, so the official APIs' adapters are tested against
//! scripted answers ([`ScriptedHttp`]) and never reach a real account.

use std::sync::{Arc, Mutex};

use crate::ChannelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub redirects: bool,
    pub limit: Option<u64>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self { method: Method::Get, url: url.into(), headers: Vec::new(), body: None, redirects: true, limit: None }
    }

    pub fn post(url: impl Into<String>, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        let headers = vec![("Content-Type".to_owned(), content_type.to_owned())];
        Self { method: Method::Post, url: url.into(), headers, body: Some(body.into()), redirects: true, limit: None }
    }

    pub fn no_redirects(mut self) -> Self {
        self.redirects = false;
        self
    }

    pub fn at_most(mut self, bytes: u64) -> Self {
        self.limit = Some(bytes);
        self
    }

    pub fn json(url: impl Into<String>, body: &serde_json::Value) -> Self {
        Self::post(url, "application/json", body.to_string())
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    pub fn bearer(self, token: &str) -> Self {
        self.header("Authorization", format!("Bearer {token}"))
    }

    pub fn multipart(url: impl Into<String>, fields: &[(&str, &str)], file: &FilePart<'_>) -> Self {
        let boundary = format!("superfik-{}", superfik_core::RunId::new());
        let mut body = Vec::new();
        for (name, value) in fields {
            body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes());
            body.extend(value.as_bytes());
            body.extend(b"\r\n");
        }
        let filename = file.filename.replace(['"', '\r', '\n'], "_");
        body.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{filename}\"\r\n\
                 Content-Type: {}\r\n\r\n",
                file.field, file.mime
            )
            .as_bytes(),
        );
        body.extend(file.bytes);
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        Self::post(url, &format!("multipart/form-data; boundary={boundary}"), body)
    }

    pub fn text(&self) -> String {
        self.body.as_deref().map(String::from_utf8_lossy).unwrap_or_default().into_owned()
    }
}

pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub struct FilePart<'a> {
    pub field: &'a str,
    pub filename: &'a str,
    pub mime: &'a str,
    pub bytes: &'a [u8],
}

impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let url = self.url.split('?').next().unwrap_or_default();
        write!(f, "{:?} {url} ({} bytes)", self.method, self.body.as_ref().map_or(0, Vec::len))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok_json(body: &serde_json::Value) -> Self {
        Self { status: 200, body: body.to_string().into_bytes() }
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }

    pub fn ok(self) -> Result<serde_json::Value, ChannelError> {
        if self.is_success() {
            return Ok(self.json());
        }
        let body = self.json();
        let said = [&body["error"]["message"], &body["detail"], &body["title"], &body["message"]]
            .into_iter()
            .find_map(serde_json::Value::as_str)
            .map(str::to_owned);
        Err(ChannelError::Failed(said.unwrap_or_else(|| format!("the API answered {}", self.status))))
    }
}

pub trait Http: Send + Sync {
    fn call(&self, request: HttpRequest) -> Result<HttpResponse, ChannelError>;
}

#[derive(Clone, Default)]
pub struct ScriptedHttp {
    answers: Arc<Mutex<std::collections::VecDeque<HttpResponse>>>,
    calls: Arc<Mutex<Vec<HttpRequest>>>,
}

impl std::fmt::Debug for ScriptedHttp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptedHttp").field("calls", &self.calls()).finish_non_exhaustive()
    }
}

impl ScriptedHttp {
    pub fn new(answers: impl IntoIterator<Item = HttpResponse>) -> Self {
        let http = Self::default();
        for answer in answers {
            http.answer(answer);
        }
        http
    }

    pub fn answer(&self, answer: HttpResponse) {
        if let Ok(mut answers) = self.answers.lock() {
            answers.push_back(answer);
        }
    }

    pub fn calls(&self) -> Vec<HttpRequest> {
        self.calls.lock().map(|calls| calls.clone()).unwrap_or_default()
    }
}

impl Http for ScriptedHttp {
    fn call(&self, request: HttpRequest) -> Result<HttpResponse, ChannelError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(request);
        }
        let next = self.answers.lock().ok().and_then(|mut answers| answers.pop_front());
        Ok(next.unwrap_or(HttpResponse { status: 500, body: Vec::new() }))
    }
}

#[derive(Debug, Clone)]
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl Default for UreqHttp {
    fn default() -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(60)))
            .build();
        Self { agent: config.into() }
    }
}

impl Http for UreqHttp {
    fn call(&self, request: HttpRequest) -> Result<HttpResponse, ChannelError> {
        let failed = |error: ureq::Error| ChannelError::Failed(error.to_string());
        let redirects = if request.redirects { 10 } else { 0 };
        let mut response = match (request.method, request.body) {
            (Method::Get, _) => {
                let mut call = self.agent.get(&request.url).config().max_redirects(redirects).build();
                for (name, value) in &request.headers {
                    call = call.header(name, value);
                }
                call.call().map_err(failed)?
            }
            (Method::Delete, _) => {
                let mut call = self.agent.delete(&request.url).config().max_redirects(redirects).build();
                for (name, value) in &request.headers {
                    call = call.header(name, value);
                }
                call.call().map_err(failed)?
            }
            (Method::Post, body) => {
                let mut call = self.agent.post(&request.url).config().max_redirects(redirects).build();
                for (name, value) in &request.headers {
                    call = call.header(name, value);
                }
                call.send(body.unwrap_or_default()).map_err(failed)?
            }
        };
        let status = response.status().as_u16();
        let limit = request.limit.unwrap_or(32 * 1024 * 1024);
        let body = response.body_mut().with_config().limit(limit).read_to_vec().map_err(failed)?;
        Ok(HttpResponse { status, body })
    }
}
