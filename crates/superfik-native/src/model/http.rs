//! HTTP behind a trait, so providers are tested against recorded streams.

use std::io::{BufRead, BufReader, Cursor};

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn BufRead + Send>,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response").field("status", &self.status).finish_non_exhaustive()
    }
}

impl Response {
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.clone())
    }

    pub fn text(mut self) -> String {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut self.body, &mut text);
        text
    }
}

pub trait Http: Send + Sync {
    fn post_stopped(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Response, super::ModelError> {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(super::ModelError::Cancelled);
        }
        self.post_model(url, headers, body)
    }
    fn get(&self, _url: &str, _headers: &[(String, String)]) -> Result<Response, String> {
        Err("GET is not supported by this HTTP transport".into())
    }

    fn post_model(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, super::ModelError> {
        self.post(url, headers, body).map_err(super::ModelError::Network)
    }

    fn post_bytes(&self, url: &str, headers: &[(String, String)], body: &[u8]) -> Result<Response, String> {
        let body =
            std::str::from_utf8(body).map_err(|_| "binary HTTP bodies are unsupported by this transport".to_owned())?;
        self.post(url, headers, body)
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String>;
}

#[derive(Debug, Default)]
pub struct UreqHttp;

impl Http for UreqHttp {
    fn post_stopped(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Response, super::ModelError> {
        super::cancellable_http::post(url, headers, body, stop)
    }
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<Response, String> {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(15)))
            .build()
            .new_agent();
        let mut request = agent.get(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.call().map_err(|error| error.to_string())?;
        Ok(Response {
            status: response.status().as_u16(),
            headers: Vec::new(),
            body: Box::new(BufReader::new(response.into_body().into_reader())),
        })
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        self.post_bytes(url, headers, body.as_bytes())
    }

    fn post_bytes(&self, url: &str, headers: &[(String, String)], body: &[u8]) -> Result<Response, String> {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(std::time::Duration::from_secs(15)))
            .build()
            .new_agent();
        let mut request = agent.post(url);
        if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("content-type")) {
            request = request.header("content-type", "application/json");
        }
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.send(body).map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| Some((name.as_str().to_ascii_lowercase(), value.to_str().ok()?.to_owned())))
            .collect();
        let reader = response.into_body().into_reader();
        Ok(Response { status, headers, body: Box::new(BufReader::new(reader)) })
    }
}

pub fn read_line_stopped(
    body: &mut dyn BufRead,
    line: &mut String,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<usize, super::ModelError> {
    let before = line.len();
    loop {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(super::ModelError::Cancelled);
        }
        match body.read_line(line) {
            Ok(_) => return Ok(line.len() - before),
            Err(error) if matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => {
                continue;
            }
            Err(error) => return Err(super::ModelError::Network(error.to_string())),
        }
    }
}
impl Response {
    pub fn text_stopped(mut self, stop: &std::sync::atomic::AtomicBool) -> Result<String, super::ModelError> {
        let mut text = String::new();
        while read_line_stopped(self.body.as_mut(), &mut text, stop)? > 0 {}
        Ok(text)
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Default)]
pub struct ScriptedHttp {
    answers: std::sync::Mutex<std::collections::VecDeque<(u16, String)>>,
    sent: std::sync::Mutex<Vec<Sent>>,
}

#[cfg(any(test, feature = "test-support"))]
impl ScriptedHttp {
    pub fn new(answers: impl IntoIterator<Item = (u16, String)>) -> Self {
        Self { answers: std::sync::Mutex::new(answers.into_iter().collect()), sent: std::sync::Mutex::default() }
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.sent.lock().map(|sent| sent.clone()).unwrap_or_default()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Http for ScriptedHttp {
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<Response, String> {
        self.post(url, headers, "")
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push(Sent { url: url.to_owned(), headers: headers.to_vec(), body: body.to_owned() });
        }
        let (status, text) = self
            .answers
            .lock()
            .ok()
            .and_then(|mut answers| answers.pop_front())
            .ok_or_else(|| "no scripted answer left".to_owned())?;
        Ok(Response { status, headers: Vec::new(), body: Box::new(Cursor::new(text.into_bytes())) })
    }
}

pub struct Stopped<'a> {
    pub http: &'a dyn Http,
    pub stop: &'a std::sync::atomic::AtomicBool,
}
impl Http for Stopped<'_> {
    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        let response = self.http.post_stopped(url, headers, body, self.stop).map_err(|e| e.to_string())?;
        let status = response.status;
        let headers = response.headers.clone();
        let text = response.text_stopped(self.stop).map_err(|e| e.to_string())?;
        if text.len() > 8 * 1024 * 1024 {
            return Err("authentication response exceeds size limit".into());
        }
        Ok(Response { status, headers, body: Box::new(Cursor::new(text.into_bytes())) })
    }
}

impl std::fmt::Debug for Stopped<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoppedHttp")
    }
}
