mod body;
mod stream;

use std::sync::Arc;
use std::time::Duration;

pub use body::body;
pub use stream::Folder;

use crate::model::http::Http;
use crate::model::retry::{Attempt, Failed, refusal, with_retries};
use crate::model::{BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest};

pub const NAME: &str = "chat";

pub const ORIGIN: crate::transcript::ReasoningOrigin = crate::transcript::ReasoningOrigin::of("Local");

pub fn open(http: Arc<dyn Http>, _model: Option<&domain::ModelOption>) -> Arc<dyn ModelApi> {
    Arc::new(Chat::new(http, Arc::new(SameModel)))
}

struct SameModel;

impl Endpoints for SameModel {
    fn endpoint(&self, model: &str) -> Result<Endpoint, ModelError> {
        Ok(Endpoint { base_url: String::new(), model: model.to_owned() })
    }
}

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
}

impl Attempt for Chat {
    fn once<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, Failed>> {
        Box::pin(async move {
            let endpoint = self.endpoints.endpoint(&request.model)?;
            let url = format!("{}/v1/chat/completions", endpoint.base_url.trim_end_matches('/'));
            let sent = body(request, &endpoint.model).to_string();
            let response = self.http.post_streaming(&url, &[], &sent, cancel).await?;
            if response.status != 200 {
                return Err(refusal(response, cancel, status_error).await);
            }
            Ok(crate::model::sse_read::read_sse(response, Folder::new(), on, cancel).await?)
        })
    }
}

impl crate::model::sse_read::SseFold for Folder {
    fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        Folder::event(self, data, on)
    }
    fn finish(self, on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        Folder::finish(self, on)
    }
}

impl ModelApi for Chat {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(with_retries("The model", self.backoff, self, request, on, cancel))
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

#[cfg(test)]
mod wire_tests;
