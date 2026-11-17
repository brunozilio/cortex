use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use platform::sync::Locked;
use serde_json::{Value, json};

use super::sse_listen::{Link, Listening, Waiting, open};
use super::{ANSWER_WAIT, Transport};
use crate::model::http::Http;

pub struct SseTransport {
    link: Arc<Link>,
    waiting: Waiting,
    closed: Arc<AtomicBool>,
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
        self.closed.store(true, Ordering::Relaxed);
    }
}

impl SseTransport {
    pub fn connect(
        name: &str,
        url: &str,
        headers: &[(String, String)],
        http: Arc<dyn Http>,
        stop: &AtomicBool,
        listener: Option<super::Listener>,
    ) -> Result<Self, String> {
        let opened = open(name, url, headers, &http, stop)?;
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
            closed: Arc::default(),
            down: Arc::default(),
            greeting: Arc::default(),
            answer_wait: ANSWER_WAIT,
        };
        let listening = Listening {
            link: Arc::clone(&transport.link),
            waiting: Arc::clone(&transport.waiting),
            closed: Arc::clone(&transport.closed),
            down: Arc::clone(&transport.down),
            greeting: Arc::clone(&transport.greeting),
            listener,
        };
        platform::named::spawn("mcp-sse", move || listening.run(opened.stream, opened.decoder));
        Ok(transport)
    }

    fn forget(&self, id: &str) {
        self.waiting.locked().remove(id);
    }

    fn closed_its_stream(&self) -> String {
        format!("{} closed its stream", self.link.name)
    }
}

impl Transport for SseTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.request_stopped(message, &platform::Stop::new())
    }

    fn request_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<Value, String> {
        if message["method"] == "initialize" {
            *self.greeting.locked() = Some(message.clone());
        }
        if self.down.load(Ordering::Relaxed) {
            return Err(self.closed_its_stream());
        }
        let id = message.get("id").map(Value::to_string).unwrap_or_default();
        let (send, receive): (Sender<Value>, Receiver<Value>) = channel();
        self.waiting.locked().insert(id.clone(), send);
        if let Err(error) = self.link.post(message, stop) {
            self.forget(&id);
            return Err(format!("{}: {error}", self.link.name));
        }
        let deadline = Instant::now() + self.answer_wait;
        loop {
            let stopped = stop.load(Ordering::Relaxed);
            if stopped || self.down.load(Ordering::Relaxed) {
                if let Ok(reply) = receive.try_recv() {
                    return Ok(reply);
                }
                self.forget(&id);
                return Err(if stopped { "cancelled".into() } else { self.closed_its_stream() });
            }
            if Instant::now() > deadline {
                self.forget(&id);
                return Err(format!("{} did not answer in {}s", self.link.name, self.answer_wait.as_secs()));
            }
            match receive.recv_timeout(Duration::from_millis(25)) {
                Ok(reply) => return Ok(reply),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(self.closed_its_stream()),
            }
        }
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        self.notify_stopped(message, &platform::Stop::new())
    }

    fn notify_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<(), String> {
        self.link.post(message, stop).map_err(|error| format!("{}: {error}", self.link.name))
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
