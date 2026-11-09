//! Chat Completions, streamed: the API local model servers
//! (`mlx_lm.server`) speak. No key; the server for a model is found
//! (and started, if it must be) through [`Endpoints`].

mod body;
mod stream;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub use body::body;
pub use stream::Folder;

use super::http::Http;
use super::sse::SseDecoder;
use super::{Delta, ModelApi, ModelError, Reply, TurnRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
}

pub trait Endpoints: Send + Sync {
    fn endpoint(&self, model: &str) -> Result<Endpoint, ModelError>;
}

const ATTEMPTS: u32 = 4;

pub struct Chat {
    http: Arc<dyn Http>,
    endpoints: Arc<dyn Endpoints>,
    backoff: Duration,
}

impl std::fmt::Debug for Chat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chat").finish_non_exhaustive()
    }
}

impl Chat {
    pub fn new(http: Arc<dyn Http>, endpoints: Arc<dyn Endpoints>) -> Self {
        Self { http, endpoints, backoff: Duration::from_secs(2) }
    }

    pub fn without_backoff(mut self) -> Self {
        self.backoff = Duration::ZERO;
        self
    }

    fn once(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let endpoint = self.endpoints.endpoint(&request.model)?;
        let url = format!("{}/v1/chat/completions", endpoint.base_url.trim_end_matches('/'));
        let response = self.http.post_stopped(&url, &[], &body(request, &endpoint.model).to_string(), stop)?;
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
        folder.finish(on)
    }
}

impl ModelApi for Chat {
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
                    tracing::info!(%error, attempt, "the local model is not ready, retrying");
                    std::thread::sleep(wait);
                    wait *= 2;
                }
                other => return other,
            }
        }
        Err(ModelError::Overloaded("the local model did not answer after retrying".into()))
    }
}

fn status_error(status: u16, text: &str) -> ModelError {
    let reason = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .filter(|value| value["error"].is_object())
        .map_or_else(|| text.to_owned(), |value| stream::message(&value["error"]));
    match status {
        503 | 429 => ModelError::Overloaded(reason),
        _ => ModelError::Provider(reason),
    }
}
