//! HTTP behind a trait, so providers are tested against recorded streams.

use std::io::{BufRead, Cursor};

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
    /// A GET whose body streams until `stop` (an event stream).
    fn get_stream(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Response, super::ModelError> {
        Err(super::ModelError::Network("streaming GET is not supported by this HTTP transport".into()))
    }

    fn post_model(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, super::ModelError> {
        self.post(url, headers, body).map_err(super::ModelError::Network)
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String>;
}

/// The network. Model turns stream and stop with the run; short calls
/// (sign-in, catalogs) are read whole within [`SHORT_TIMEOUT`].
#[derive(Debug, Default)]
pub struct NetHttp;

/// How long a sign-in, catalog or other short call may take.
pub const SHORT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

impl NetHttp {
    fn short(url: &str, headers: &[(String, String)], body: Option<&str>) -> Result<Response, String> {
        let never = std::sync::atomic::AtomicBool::new(false);
        // The bare reason: callers say themselves that the provider was not reached.
        let response =
            super::cancellable_http::send(url, headers, body, &never, SHORT_TIMEOUT).map_err(|error| match error {
                super::ModelError::Network(reason) => reason,
                error => error.to_string(),
            })?;
        let (status, headers) = (response.status, response.headers.clone());
        let text = response.text_stopped(&never).map_err(|error| error.to_string())?;
        Ok(Response { status, headers, body: Box::new(Cursor::new(text.into_bytes())) })
    }
}

impl Http for NetHttp {
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
        Self::short(url, headers, None)
    }

    fn get_stream(
        &self,
        url: &str,
        headers: &[(String, String)],
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Response, super::ModelError> {
        // An MCP server's stream lasts as long as the run that opened it.
        super::cancellable_http::send(url, headers, None, stop, std::time::Duration::from_secs(12 * 3600))
    }

    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        Self::short(url, headers, Some(body))
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
