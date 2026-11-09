//! Seam 3: the loopback server admits a run's token, routes its requests to
//! the upstream of the token's provider, and answers as Anthropic would.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use superfik_core::ModelProvider;
use superfik_gateway::messages::MessagesRequest;
use superfik_gateway::server::{Gateway, Upstream, UpstreamError, Upstreams};
use superfik_gateway::stream::{ChatStream, Event};

#[derive(Default)]
struct Echo {
    reply: &'static str,
    asked: Mutex<Vec<String>>,
}

impl Upstream for Echo {
    fn messages(&self, request: &MessagesRequest, emit: &mut dyn FnMut(Event)) -> Result<(), UpstreamError> {
        self.asked.lock().expect("asked").push(request.model.clone());
        let mut stream = ChatStream::new(&request.model);
        let mut events = stream.start();
        events.extend(
            stream.push(&json!({ "choices": [{ "delta": { "content": self.reply }, "finish_reason": "stop" }] })),
        );
        events.extend(stream.finish());
        events.into_iter().for_each(emit);
        Ok(())
    }
}

struct Down;

impl Upstream for Down {
    fn messages(&self, _: &MessagesRequest, _: &mut dyn FnMut(Event)) -> Result<(), UpstreamError> {
        Err(UpstreamError::Unavailable("the model is still loading".into()))
    }
}

fn post(url: &str, token: &str, body: &Value) -> (u16, String) {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let mut response =
        agent.post(url).header("authorization", &format!("Bearer {token}")).send_json(body).expect("request");
    let status = response.status().as_u16();
    (status, response.body_mut().read_to_string().expect("body"))
}

fn ask(stream: bool) -> Value {
    json!({ "model": "qwen", "max_tokens": 32, "stream": stream, "messages": [{ "role": "user", "content": "hi" }] })
}

fn gateway(local: Arc<dyn Upstream>, codex: Arc<dyn Upstream>) -> Gateway {
    Gateway::start(Upstreams::new().with(ModelProvider::Local, local).with(ModelProvider::OpenAi, codex))
        .expect("start")
}

#[test]
fn a_streamed_request_reaches_the_upstream_of_its_tokens_provider() {
    let local = Arc::new(Echo { reply: "from local", ..Echo::default() });
    let codex = Arc::new(Echo { reply: "from codex", ..Echo::default() });
    let gateway = gateway(local.clone(), codex.clone());
    let access = gateway.admit(ModelProvider::OpenAi);
    assert!(access.base_url.starts_with("http://127.0.0.1:"));
    let (status, body) = post(&format!("{}/v1/messages?beta=true", access.base_url), &access.token, &ask(true));
    assert_eq!(status, 200);
    assert!(body.starts_with("event: message_start\n"), "{body}");
    assert!(body.contains("\"text\":\"from codex\""));
    assert!(body.trim_end().ends_with("data: {\"type\":\"message_stop\"}"));
    assert_eq!(*codex.asked.lock().expect("asked"), ["qwen"]);
    assert!(local.asked.lock().expect("asked").is_empty());
}

#[test]
fn a_request_that_does_not_stream_gets_the_whole_message() {
    let echo = Arc::new(Echo { reply: "whole", ..Echo::default() });
    let gateway = gateway(echo.clone(), echo);
    let access = gateway.admit(ModelProvider::Local);
    let (status, body) = post(&format!("{}/v1/messages", access.base_url), &access.token, &ask(false));
    assert_eq!(status, 200);
    let message: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(message["content"], json!([{ "type": "text", "text": "whole" }]));
    assert_eq!(message["stop_reason"], "end_turn");
}

#[test]
fn unknown_and_revoked_tokens_are_refused() {
    let echo = Arc::new(Echo::default());
    let gateway = gateway(echo.clone(), echo.clone());
    let access = gateway.admit(ModelProvider::Local);
    let url = format!("{}/v1/messages", access.base_url);
    let (status, body) = post(&url, "guessed", &ask(false));
    assert_eq!(status, 401);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["type"], "authentication_error");
    gateway.revoke(&access.token);
    assert_eq!(post(&url, &access.token, &ask(false)).0, 401);
    assert!(echo.asked.lock().expect("asked").is_empty());
}

#[test]
fn an_upstream_that_cannot_answer_is_an_overloaded_error() {
    let gateway = gateway(Arc::new(Down), Arc::new(Down));
    let access = gateway.admit(ModelProvider::Local);
    let (status, body) = post(&format!("{}/v1/messages", access.base_url), &access.token, &ask(true));
    assert_eq!(status, 529);
    let error: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(error["error"], json!({ "type": "overloaded_error", "message": "the model is still loading" }));
}

#[test]
fn a_body_that_is_not_a_messages_request_is_invalid() {
    let echo = Arc::new(Echo::default());
    let gateway = gateway(echo.clone(), echo);
    let access = gateway.admit(ModelProvider::Local);
    let (status, body) = post(&format!("{}/v1/messages", access.base_url), &access.token, &json!({ "nope": 1 }));
    assert_eq!(status, 400);
    assert_eq!(serde_json::from_str::<Value>(&body).expect("json")["error"]["type"], "invalid_request_error");
}

#[test]
fn counting_tokens_gives_an_estimate() {
    let echo = Arc::new(Echo::default());
    let gateway = gateway(echo.clone(), echo);
    let access = gateway.admit(ModelProvider::Local);
    let (status, body) = post(&format!("{}/v1/messages/count_tokens", access.base_url), &access.token, &ask(false));
    assert_eq!(status, 200);
    let tokens = serde_json::from_str::<Value>(&body).expect("json")["input_tokens"].as_u64().unwrap_or_default();
    assert!(tokens > 0);
}
