mod body;
mod stream;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub use body::{body, effort};
pub use stream::Folder;

use crate::model::http::Http;
use crate::model::{Delta, KeySource, ModelApi, ModelError, Reply, TurnRequest};

pub const NAME: &str = "responses";

pub const ORIGIN: crate::transcript::ReasoningOrigin = crate::transcript::ReasoningOrigin::of("OpenAi");

pub fn open(http: Arc<dyn Http>, _model: Option<&domain::ModelOption>) -> Arc<dyn ModelApi> {
    Arc::new(Responses::new(http, Arc::new(domain::provider::ProviderKey::new("transport-owned"))))
}


pub const API: &str = "https://api.openai.com/v1";

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
            (
                "user-agent".to_owned(),
                format!("{brand_command}/{}", env!("CARGO_PKG_VERSION"), brand_command = brand::COMMAND),
            ),
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
        crate::model::sse_read::read_sse(response, Folder::new(), on, stop)
    }
}

impl crate::model::sse_read::SseFold for Folder {
    fn event(&mut self, data: &str, on: &mut dyn FnMut(Delta)) -> Result<(), ModelError> {
        Folder::event(self, data, on)
    }
    fn finish(self, _on: &mut dyn FnMut(Delta)) -> Result<Reply, ModelError> {
        Folder::finish(self)
    }
}

impl ModelApi for Responses {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        crate::model::retry::with_retries("OpenAI", self.backoff, stop, on, |on| self.once(request, on, stop))
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

#[cfg(test)]
mod wire_tests;
