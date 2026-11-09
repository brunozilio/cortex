//! OpenAI's Responses API, streamed, with the owner's API key.

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

pub const API: &str = "https://api.openai.com/v1";

const ATTEMPTS: u32 = 4;

pub struct Responses {
    http: Arc<dyn Http>,
    keys: Arc<dyn KeySource>,
    backoff: Duration,
}

impl std::fmt::Debug for Responses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Responses").finish_non_exhaustive()
    }
}

impl Responses {
    pub fn new(http: Arc<dyn Http>, keys: Arc<dyn KeySource>) -> Self {
        Self { http, keys, backoff: Duration::from_secs(2) }
    }

    pub fn without_backoff(mut self) -> Self {
        self.backoff = Duration::ZERO;
        self
    }

    fn target(&self) -> Result<(String, Vec<(String, String)>), ModelError> {
        let key = self.keys.key()?;
        let headers = vec![
            ("user-agent".to_owned(), format!("superfik/{}", env!("CARGO_PKG_VERSION"))),
            ("authorization".to_owned(), format!("Bearer {}", key.secret())),
        ];
        Ok((format!("{API}/responses"), headers))
    }

    fn once(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let (url, headers) = self.target()?;
        let response = self.http.post_stopped(&url, &headers, &body(request).to_string(), stop)?;
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

impl ModelApi for Responses {
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
                    tracing::warn!(%error, attempt, "OpenAI is busy, retrying");
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
    let error = parsed.as_ref().map(|value| if value["error"].is_object() { &value["error"] } else { value });
    match (status, error) {
        (401 | 403, _) => ModelError::Unauthorized(text.to_owned()),
        (_, Some(error)) if error["message"].is_string() => match stream::failure(error) {
            ModelError::Provider(message) if status == 429 || status >= 500 => ModelError::Overloaded(message),
            other => other,
        },
        (429 | 500..=599, _) => ModelError::Overloaded(format!("HTTP {status}")),
        _ => ModelError::Provider(format!("HTTP {status}: {text}")),
    }
}
