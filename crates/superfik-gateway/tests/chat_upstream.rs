//! `ChatUpstream` asks where a model is served, sends it the translated
//! request under the name that server knows it by, and relays the answer.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use superfik_gateway::local::{ChatUpstream, Endpoint, Endpoints};
use superfik_gateway::messages::MessagesRequest;
use superfik_gateway::server::{Upstream, UpstreamError};
use superfik_gateway::stream::message_from;

fn serve(status: u16, body: &'static str) -> (String, Arc<Mutex<Vec<Value>>>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let port = server.server_addr().to_ip().map(|a| a.port()).expect("port");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let keep = seen.clone();
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut text = String::new();
            let _ = request.as_reader().read_to_string(&mut text);
            keep.lock()
                .expect("seen")
                .push(json!({ "path": request.url(), "body": serde_json::from_str::<Value>(&text).ok() }));
            let _ = request.respond(tiny_http::Response::from_string(body).with_status_code(status));
        }
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

struct At(Result<Endpoint, &'static str>);

impl Endpoints for At {
    fn endpoint(&self, _model: &str) -> Result<Endpoint, UpstreamError> {
        self.0.clone().map_err(|why| UpstreamError::Unavailable(why.into()))
    }
}

fn request() -> MessagesRequest {
    serde_json::from_value(json!({
        "model": "mlx-community/Qwen3.8-27B-8bit",
        "max_tokens": 800,
        "stream": true,
        "messages": [{ "role": "user", "content": "List the files." }]
    }))
    .expect("request")
}

fn answer(upstream: &ChatUpstream) -> Result<Vec<superfik_gateway::stream::Event>, UpstreamError> {
    let mut events = Vec::new();
    upstream.messages(&request(), &mut |event| events.push(event))?;
    Ok(events)
}

#[test]
fn the_model_is_asked_for_under_the_name_its_server_knows() {
    let (base_url, seen) = serve(200, include_str!("fixtures/mlx_tool_call.sse"));
    let endpoint = Endpoint { base_url, model: "/models/mlx-community/Qwen3.8-27B-8bit".into() };
    let upstream = ChatUpstream::new(Arc::new(At(Ok(endpoint))));
    let events = answer(&upstream).expect("answer");
    let message = message_from(&events);
    assert_eq!(message["model"], "mlx-community/Qwen3.8-27B-8bit");
    assert_eq!(message["stop_reason"], "tool_use");
    let seen = seen.lock().expect("seen");
    assert_eq!(seen[0]["path"], "/v1/chat/completions");
    assert_eq!(seen[0]["body"]["model"], "/models/mlx-community/Qwen3.8-27B-8bit");
    assert_eq!(seen[0]["body"]["stream"], true);
}

#[test]
fn a_model_still_loading_is_unavailable() {
    let upstream = ChatUpstream::new(Arc::new(At(Err("Qwen3.8 27B is loading"))));
    assert!(matches!(answer(&upstream), Err(UpstreamError::Unavailable(why)) if why.contains("loading")));
}

#[test]
fn a_refusal_from_the_server_carries_its_reason() {
    let (base_url, _) = serve(404, r#"{"error": {"message": "no such model"}}"#);
    let upstream = ChatUpstream::new(Arc::new(At(Ok(Endpoint { base_url, model: "m".into() }))));
    assert!(matches!(answer(&upstream), Err(UpstreamError::Invalid(why)) if why.contains("no such model")));
}
