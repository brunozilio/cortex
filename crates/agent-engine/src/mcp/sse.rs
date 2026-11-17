use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use platform::sync::Locked;
use serde_json::{Value, json};

use super::sse_listen::{Link, Listening, Waiting, open};
use super::{ANSWER_WAIT, McpError, Transport};
use crate::model::CancellationToken;
use crate::model::http::Http;

pub struct SseTransport {
    link: Arc<Link>,
    waiting: Waiting,
    closed: CancellationToken,
    down: Arc<AtomicBool>,
    greeting: Arc<Mutex<Option<Value>>>,
    answer_wait: Duration,
}

impl std::fmt::Debug for SseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let endpoint = self.link.endpoint.locked().clone();
        f.debug_struct("SseTransport").field("name", &self.link.name).field("endpoint", &endpoint).finish()
    }
}

impl Drop for SseTransport {
    fn drop(&mut self) {
        self.closed.cancel();
    }
}

impl SseTransport {
    pub fn connect(
        name: &str,
        url: &str,
        headers: &[(String, String)],
        http: Arc<dyn Http>,
        cancel: &CancellationToken,
        listener: Option<super::Listener>,
    ) -> Result<Self, String> {
        let runtime = platform::runtime::handle().map_err(|error| format!("{name}: {error}"))?;
        let opened = platform::runtime::block_on(open(name, url, headers, &http, cancel))
            .map_err(|error| format!("{name}: {error}"))??;
        let link = Link {
            name: name.to_owned(),
            url: url.to_owned(),
            endpoint: Mutex::new(opened.endpoint),
            headers: headers.to_vec(),
            http,
        };
        let transport = Self {
            link: Arc::new(link),
            waiting: Arc::default(),
            closed: CancellationToken::new(),
            down: Arc::default(),
            greeting: Arc::default(),
            answer_wait: ANSWER_WAIT,
        };
        let listening = Listening {
            link: Arc::clone(&transport.link),
            waiting: Arc::clone(&transport.waiting),
            closed: transport.closed.clone(),
            down: Arc::clone(&transport.down),
            greeting: Arc::clone(&transport.greeting),
            listener,
        };
        drop(runtime.spawn(listening.run(opened.stream, opened.decoder)));
        Ok(transport)
    }

    fn forget(&self, id: &str) {
        self.waiting.locked().remove(id);
    }

    fn closed_its_stream(&self) -> String {
        format!("{} closed its stream", self.link.name)
    }

    async fn requested(&self, message: &Value, cancel: &CancellationToken) -> Result<Value, McpError> {
        if message["method"] == "initialize" {
            *self.greeting.locked() = Some(message.clone());
        }
        let id = message.get("id").map(Value::to_string).unwrap_or_default();
        let (send, receive) = tokio::sync::oneshot::channel();
        self.waiting.locked().insert(id.clone(), send);
        if self.down.load(Ordering::SeqCst) {
            self.forget(&id);
            return Err(self.closed_its_stream().into());
        }
        if let Err(error) = self.link.post(message, cancel).await {
            self.forget(&id);
            return Err(format!("{}: {error}", self.link.name).into());
        }
        let answered = tokio::select! {
            biased;
            reply = receive => reply.map_err(|_| self.closed_its_stream().into()),
            () = cancel.cancelled() => Err(McpError::Cancelled),
            () = tokio::time::sleep(self.answer_wait) => {
                Err(format!("{} did not answer in {}s", self.link.name, self.answer_wait.as_secs()).into())
            }
        };
        if answered.is_err() {
            self.forget(&id);
        }
        answered
    }
}

impl Transport for SseTransport {
    fn request(&self, message: &Value) -> Result<Value, McpError> {
        self.request_stopped(message, &platform::Stop::new())
    }

    fn request_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<Value, McpError> {
        platform::runtime::block_on(self.requested(message, stop.token()))
            .unwrap_or_else(|error| Err(error.to_string().into()))
    }

    fn notify(&self, message: &Value) -> Result<(), McpError> {
        self.notify_stopped(message, &platform::Stop::new())
    }

    fn notify_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<(), McpError> {
        platform::runtime::block_on(self.link.post(message, stop.token()))
            .unwrap_or_else(|error| Err(error.to_string()))
            .map_err(|error| format!("{}: {error}", self.link.name).into())
    }

    fn close(&self) {
        self.closed.cancel();
    }
}

pub(super) fn client_answer(id: &Value, method: &str) -> Value {
    if method == "ping" {
        json!({ "jsonrpc": "2.0", "id": id, "result": {} })
    } else {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Client method not supported" } })
    }
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
