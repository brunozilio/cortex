mod body;
mod frames;
mod stream;

use std::sync::Arc;
use std::time::Duration;

pub use body::body;
#[doc(hidden)]
pub use frames::{Frame, next as next_frame};
pub use stream::Folder;

use crate::model::http::Http;
use crate::model::retry::{Attempt, Failed, refusal, with_retries};
use crate::model::{BoxFuture, CancellationToken, ModelApi, ModelError, OnDelta, Reply, TurnRequest};

pub const NAME: &str = "event_stream";

pub fn open(http: Arc<dyn Http>, model: Option<&domain::ModelOption>) -> Arc<dyn ModelApi> {
    Arc::new(Conversation::new(http, model.and_then(|model| model.window)))
}

pub struct Conversation {
    http: Arc<dyn Http>,
    window: Option<u64>,
    backoff: Duration,
}

impl std::fmt::Debug for Conversation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conversation").field("window", &self.window).finish_non_exhaustive()
    }
}

impl Conversation {
    pub fn new(http: Arc<dyn Http>, window: Option<u64>) -> Self {
        Self { http, window, backoff: Duration::from_secs(2) }
    }

    pub fn without_backoff(mut self) -> Self {
        self.backoff = Duration::ZERO;
        self
    }
}

impl Attempt for Conversation {
    fn once<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, Failed>> {
        Box::pin(async move {
            let response = self.http.post_streaming("", &[], &body(request).to_string(), cancel).await?;
            if response.status != 200 {
                return Err(refusal(response, cancel, status_error).await);
            }
            let mut folder = Folder::new(self.window);
            let mut body = response.body;
            while let Some(frame) = frames::next(&mut body, cancel).await? {
                folder.frame(&frame, on)?;
            }
            Ok(folder.finish()?)
        })
    }
}

impl ModelApi for Conversation {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(with_retries("Kiro", self.backoff, self, request, on, cancel))
    }
}

fn status_error(status: u16, text: &str) -> ModelError {
    let body: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let kind = body["__type"].as_str().unwrap_or_default();
    match (status, stream::error(kind, &body)) {
        (_, ModelError::Provider(_)) if status == 401 || status == 403 => ModelError::Unauthorized(text.to_owned()),
        (429 | 500..=599, ModelError::Provider(message)) => ModelError::Overloaded(message),
        (_, ModelError::Provider(message)) => ModelError::Provider(format!("HTTP {status}: {message}")),
        (_, error) => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::http::Response;
    use crate::transcript::{Message, StopReason};
    use std::sync::Mutex;

    struct Recorded(Mutex<Vec<(u16, Vec<u8>)>>, Mutex<Vec<String>>);

    impl Http for Recorded {
        fn post(&self, _url: &str, _headers: &[(String, String)], body: &str) -> Result<Response, String> {
            self.1.lock().map_err(|e| e.to_string())?.push(body.to_owned());
            let (status, bytes) = self.0.lock().map_err(|e| e.to_string())?.remove(0);
            Ok(Response { status, headers: Vec::new(), body: bytes.into() })
        }
    }

    fn request() -> TurnRequest {
        TurnRequest {
            model: "kiro-subscription:auto".into(),
            system: "s".into(),
            messages: vec![Message::user("hi")],
            tools: Vec::new(),
            effort: domain::Effort::Medium,
            max_tokens: 1024,
        }
    }

    fn run(answers: Vec<(u16, Vec<u8>)>) -> (Result<Reply, ModelError>, usize) {
        let http = Arc::new(Recorded(Mutex::new(answers), Mutex::new(Vec::new())));
        let api = Conversation::new(http.clone(), None).without_backoff();
        let reply = crate::model::BlockingTurn::turn_blocking(&api, &request(), &mut |_| {}, &CancellationToken::new());
        let sent = http.1.lock().map(|sent| sent.len()).unwrap_or_default();
        (reply, sent)
    }

    #[test]
    fn a_throttled_turn_is_retried_then_answered() {
        let busy = br#"{"__type":"com.amazon.kiro#ThrottlingException","message":"slow"}"#.to_vec();
        let (reply, sent) = run(vec![(429, busy), (200, include_bytes!("testdata/text.bin").to_vec())]);
        assert_eq!(sent, 2);
        assert_eq!(reply.map(|r| r.stop), Ok(StopReason::EndTurn));
    }

    #[test]
    fn a_refused_model_is_not_retried() {
        let invalid = br#"{"__type":"com.amazon.kiro.runtimeservice#ValidationException","message":"Invalid model ID.","reason":"INVALID_MODEL_ID"}"#;
        let (reply, sent) = run(vec![(400, invalid.to_vec())]);
        assert_eq!(sent, 1);
        assert_eq!(reply.map(|r| r.stop), Err(ModelError::Provider("HTTP 400: Invalid model ID.".into())));
    }

    #[test]
    fn a_rejected_token_signs_out() {
        let denied = br#"{"__type":"com.amazon.aws.codewhisperer#AccessDeniedException","message":"The bearer token included in the request is invalid."}"#;
        assert!(matches!(run(vec![(403, denied.to_vec())]).0, Err(ModelError::Unauthorized(_))));
    }
}
