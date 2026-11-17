mod body;
mod stream;

use std::sync::Arc;
use std::time::Duration;

pub use body::{SKIP_SIGNATURE, body};
pub use stream::Folder;

use crate::model::http::Http;
use crate::model::retry::{Attempt, Failed, refusal, with_retries};
use crate::model::{BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use crate::transcript::ReasoningOrigin;

pub const NAME: &str = "cloud_code";

pub const ORIGIN: ReasoningOrigin = ReasoningOrigin::of("Antigravity");

pub fn open(http: Arc<dyn Http>, _model: Option<&domain::ModelOption>) -> Arc<dyn ModelApi> {
    Arc::new(Conversation::new(http))
}

pub struct Conversation {
    http: Arc<dyn Http>,
    backoff: Duration,
}

impl std::fmt::Debug for Conversation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conversation").finish_non_exhaustive()
    }
}

impl Conversation {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http, backoff: Duration::from_secs(2) }
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

impl ModelApi for Conversation {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(with_retries("Antigravity", self.backoff, self, request, on, cancel))
    }
}

fn status_error(status: u16, text: &str) -> ModelError {
    let value: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let error = if value[0]["error"].is_object() { &value[0]["error"] } else { &value["error"] };
    if error.is_object() {
        return stream::error(status, error);
    }
    stream::error(status, &serde_json::json!({"message": text.trim()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::http::Response;
    use crate::transcript::{Message, StopReason};
    use std::sync::Mutex;

    struct Recorded(Mutex<Vec<(u16, String)>>, Mutex<Vec<String>>);

    impl Http for Recorded {
        fn post(&self, _url: &str, _headers: &[(String, String)], body: &str) -> Result<Response, String> {
            self.1.lock().map_err(|e| e.to_string())?.push(body.to_owned());
            let (status, text) = self.0.lock().map_err(|e| e.to_string())?.remove(0);
            Ok(Response { status, headers: Vec::new(), body: text.into() })
        }
    }

    fn run(answers: Vec<(u16, &str)>) -> (Result<Reply, ModelError>, usize) {
        let answers = answers.into_iter().map(|(status, text)| (status, text.to_owned())).collect();
        let http = Arc::new(Recorded(Mutex::new(answers), Mutex::new(Vec::new())));
        let api = Conversation::new(http.clone()).without_backoff();
        let request = TurnRequest {
            model: "antigravity-subscription:gemini-3.8-flash-high".into(),
            system: "s".into(),
            messages: vec![Message::user("hi")],
            tools: Vec::new(),
            effort: domain::Effort::Medium,
            max_tokens: 1024,
        };
        let reply = crate::model::BlockingTurn::turn_blocking(&api, &request, &mut |_| {}, &CancellationToken::new());
        let sent = http.1.lock().map(|sent| sent.len()).unwrap_or_default();
        (reply, sent)
    }

    const ANSWER: &str = "data: {\"response\":{\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hello\"}]},\"finishReason\":\"STOP\"}]}}\n\n";

    #[test]
    fn a_throttled_turn_is_retried_then_answered() {
        let busy = r#"{"error":{"code":429,"message":"Resource has been exhausted","status":"RESOURCE_EXHAUSTED"}}"#;
        let (reply, sent) = run(vec![(429, busy), (200, ANSWER)]);
        assert_eq!(sent, 2);
        assert_eq!(reply.map(|r| (r.stop, r.message.text())), Ok((StopReason::EndTurn, "hello".into())));
    }

    #[test]
    fn a_refused_request_is_not_retried_and_names_the_reason() {
        let invalid = r#"[{"error":{"code":400,"message":"Unknown model.","status":"INVALID_ARGUMENT"}}]"#;
        let (reply, sent) = run(vec![(400, invalid)]);
        assert_eq!(sent, 1);
        assert_eq!(reply.map(|r| r.stop), Err(ModelError::Provider("HTTP 400: Unknown model.".into())));
    }

    #[test]
    fn a_rejected_token_signs_out() {
        let denied = r#"{"error":{"code":401,"message":"Request had invalid authentication credentials.","status":"UNAUTHENTICATED"}}"#;
        assert!(matches!(run(vec![(401, denied)]).0, Err(ModelError::Unauthorized(_))));
    }
}
