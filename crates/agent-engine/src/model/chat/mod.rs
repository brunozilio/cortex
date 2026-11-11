mod body;
mod stream;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub use body::body;
pub use stream::Folder;

use super::http::Http;
use super::{Delta, ModelApi, ModelError, Reply, TurnRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
}

pub trait Endpoints: Send + Sync {
    fn endpoint(&self, model: &str) -> Result<Endpoint, ModelError>;
}

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
        super::sse_read::read_sse(response, Folder::new(), on, stop)
    }
}

impl super::sse_read::SseFold for Folder {
    fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        Folder::event(self, data, on)
    }
    fn finish(self, on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        Folder::finish(self, on)
    }
}

impl ModelApi for Chat {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        super::retry::with_retries("The model", self.backoff, stop, on, |on| self.once(request, on, stop))
    }
}

fn status_error(status: u16, text: &str) -> ModelError {
    let reason = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .filter(|value| value["error"].is_object())
        .map_or_else(|| text.to_owned(), |value| stream::message(&value["error"]));
    match status {
        401 | 403 => ModelError::Unauthorized(reason),
        413 => ModelError::TooLong(reason),
        _ if reason.contains("context length") || reason.contains("context_length") => ModelError::TooLong(reason),
        429 | 500..=599 => ModelError::Overloaded(reason),
        _ => ModelError::Provider(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_say_what_kind_of_failure_it_was() {
        assert!(matches!(status_error(401, "{}"), ModelError::Unauthorized(_)));
        assert!(matches!(status_error(403, "no"), ModelError::Unauthorized(_)));
        assert!(matches!(status_error(413, "big"), ModelError::TooLong(_)));
        let long = r#"{"error":{"message":"This model's maximum context length is 8192 tokens"}}"#;
        assert!(matches!(status_error(400, long), ModelError::TooLong(_)));
        assert!(matches!(status_error(502, "starting"), ModelError::Overloaded(_)));
        assert!(matches!(status_error(429, "slow down"), ModelError::Overloaded(_)));
        let bad = r#"{"error":{"message":"unknown field"}}"#;
        assert!(matches!(status_error(400, bad), ModelError::Provider(why) if why == "unknown field"));
    }
}
