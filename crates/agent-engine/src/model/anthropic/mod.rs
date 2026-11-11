//! Anthropic's Messages API (`POST /v1/messages`), streamed.

mod body;
mod stream;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub use body::{body, effort};
pub use stream::Folder;

use super::http::Http;
use super::sse::SseDecoder;
use super::{Delta, KeySource, ModelApi, ModelError, Reply, TurnRequest};

pub const API: &str = "https://api.anthropic.com";
pub const VERSION: &str = "2026-10-21";

const ATTEMPTS: u32 = 4;

pub struct Messages {
    http: Arc<dyn Http>,
    keys: Arc<dyn KeySource>,
    base: String,
    backoff: Duration,
}

impl std::fmt::Debug for Messages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Messages").field("base", &self.base).finish_non_exhaustive()
    }
}

impl Messages {
    pub fn new(http: Arc<dyn Http>, keys: Arc<dyn KeySource>) -> Self {
        Self { http, keys, base: API.to_owned(), backoff: Duration::from_secs(2) }
    }

    pub fn without_backoff(mut self) -> Self {
        self.backoff = Duration::ZERO;
        self
    }

    fn headers(&self) -> Result<Vec<(String, String)>, ModelError> {
        let mut headers = vec![
            ("anthropic-version".to_owned(), VERSION.to_owned()),
            (
                "user-agent".to_owned(),
                format!("{brand_command}/{}", env!("CARGO_PKG_VERSION"), brand_command = brand::COMMAND),
            ),
        ];
        headers.push(("x-api-key".to_owned(), self.keys.key()?.secret().to_owned()));
        Ok(headers)
    }

    fn once(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let url = format!("{}/v1/messages", self.base.trim_end_matches('/'));
        let response = self.http.post_stopped(&url, &self.headers()?, &body(request).to_string(), stop)?;
        if response.status != 200 {
            return Err(status_error(response.status, &response.text_stopped(stop)?));
        }
        let mut decoder = SseDecoder::new();
        let mut folder = Folder::new();
        let mut body = response.body;
        let mut line = String::new();
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err(ModelError::Cancelled);
            }
            line.clear();
            let read = crate::model::http::read_line_stopped(body.as_mut(), &mut line, stop)?;
            let event = if read == 0 { decoder.finish() } else { decoder.line(line.trim_end_matches('\n')) };
            if let Some(event) = event {
                folder.event(&event.data, on)?;
            }
            if read == 0 {
                break;
            }
        }
        folder.finish()
    }
}

impl ModelApi for Messages {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let mut wait = self.backoff;
        let mut wrote = false;
        for attempt in 1..=ATTEMPTS {
            let mut seen = |delta: Delta| {
                wrote = true;
                on(delta);
            };
            match self.once(request, &mut seen, stop) {
                Err(error) if error.is_transient() && !wrote && attempt < ATTEMPTS => {
                    tracing::warn!(%error, attempt, "Anthropic is busy, retrying");
                    std::thread::sleep(wait);
                    wait *= 2;
                }
                other => return other,
            }
        }
        Err(ModelError::Overloaded("no answer after retrying".into()))
    }
}

fn status_error(status: u16, text: &str) -> ModelError {
    let parsed = serde_json::from_str::<serde_json::Value>(text).ok();
    match parsed.as_ref().map(|value| &value["error"]) {
        Some(error) if error.is_object() => stream::stream_error(error),
        _ => match status {
            401 | 403 => ModelError::Unauthorized(text.to_owned()),
            413 => ModelError::TooLong(text.to_owned()),
            429 | 500..=599 => ModelError::Overloaded(format!("HTTP {status}")),
            _ => ModelError::Provider(format!("HTTP {status}: {text}")),
        },
    }
}
