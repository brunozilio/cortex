mod body;
mod frames;
mod stream;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub use body::body;
pub use stream::Folder;

use super::http::Http;
use super::{Delta, ModelApi, ModelError, Reply, TurnRequest};

const ATTEMPTS: u32 = 4;

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

    fn once(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let response = self.http.post_stopped("", &[], &body(request).to_string(), stop)?;
        if response.status != 200 {
            return Err(status_error(response.status, &response.text_stopped(stop)?));
        }
        let mut folder = Folder::new(self.window);
        let mut body = response.body;
        while let Some(frame) = frames::next(body.as_mut(), stop)? {
            folder.frame(&frame, on)?;
        }
        folder.finish()
    }
}

impl ModelApi for Conversation {
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
                    tracing::warn!(%error, attempt, "Kiro is busy, retrying");
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
            Ok(Response { status, headers: Vec::new(), body: Box::new(std::io::Cursor::new(bytes)) })
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
        let reply = api.turn(&request(), &mut |_| {}, &AtomicBool::new(false));
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
