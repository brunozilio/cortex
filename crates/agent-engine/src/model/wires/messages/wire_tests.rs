use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::model::wires::messages::Messages;
use crate::model::http::ScriptedHttp;
use crate::model::{Delta, ModelApi, ModelError, TurnRequest};
use crate::transcript::{Block, Message, StopReason};
use domain::Effort;
use domain::provider::ProviderKey;
use serde_json::json;

fn request() -> TurnRequest {
    TurnRequest {
        model: "claude-opus-5-5".into(),
        system: concat!("You are ", brand::name!(), "'s agent.").into(),
        messages: vec![Message::user("Read src/main.rs")],
        tools: Vec::new(),
        effort: Effort::High,
        max_tokens: 64_000,
    }
}

fn client(http: &Arc<ScriptedHttp>, key: &str) -> Messages {
    Messages::new(http.clone(), Arc::new(ProviderKey::new(key))).without_backoff()
}

#[test]
fn folds_thinking_text_and_a_tool_call() {
    let stream = include_str!("../../../../tests/fixtures/messages_tool_use.sse");
    let http = Arc::new(ScriptedHttp::new([(200, stream.to_owned())]));
    let mut deltas = Vec::new();
    let reply = client(&http, "sk-test")
        .turn(&request(), &mut |delta| deltas.push(delta), &AtomicBool::new(false))
        .expect("reply");

    assert_eq!(reply.stop, StopReason::ToolUse);
    assert_eq!(
        reply.message.blocks,
        vec![
            Block::Thinking {
                text: "I should read it.".into(),
                signature: Some("EqQB".into()),
                origin: Some(crate::model::wires::messages::ORIGIN)
            },
            Block::Text("Let me look.".into()),
            Block::ToolUse { id: "toolu_1".into(), name: "Read".into(), input: json!({"file_path": "src/main.rs"}) },
        ]
    );
    assert_eq!(
        deltas,
        vec![
            Delta::Thinking("I should read it.".into()),
            Delta::Text("Let me ".into()),
            Delta::Text("look.".into()),
            Delta::ToolInput { name: "Read".into(), piece: "{\"file_path\": \"src/".into() },
            Delta::ToolInput { name: "Read".into(), piece: "main.rs\"}".into() },
        ]
    );
    assert_eq!((reply.usage.input, reply.usage.cache_read, reply.usage.output), (12, 4000, 87));
}

#[test]
fn sends_the_key_and_names_itself() {
    let stream = include_str!("../../../../tests/fixtures/messages_tool_use.sse").to_owned();
    let http = Arc::new(ScriptedHttp::new([(200, stream)]));
    client(&http, "sk-test").turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect("key");

    let sent = http.sent();
    let header =
        |at: usize, name: &str| sent[at].headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());
    assert_eq!(sent[0].url, "https://api.anthropic.com/v1/messages");
    assert_eq!(header(0, "x-api-key").as_deref(), Some("sk-test"));
    assert_eq!(header(0, "authorization"), None);
    assert!(header(0, "user-agent").is_some_and(|agent| agent.starts_with(concat!(brand::command!(), "/"))));
    assert!(sent.iter().all(|request| !request.headers.iter().any(|(key, _)| key == "anthropic-beta")));
}

#[test]
fn retries_when_busy_and_names_what_failed() {
    let stream = include_str!("../../../../tests/fixtures/messages_tool_use.sse").to_owned();
    let busy = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#.to_owned();
    let http = Arc::new(ScriptedHttp::new([(529, busy), (200, stream)]));
    let reply = client(&http, "k").turn(&request(), &mut |_| {}, &AtomicBool::new(false));
    assert!(reply.is_ok(), "{reply:?}");
    assert_eq!(http.sent().len(), 2);

    let refused = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
    let http = Arc::new(ScriptedHttp::new([(401, refused.to_owned())]));
    let error = client(&http, "bad").turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect_err("refused");
    assert_eq!(error, ModelError::Unauthorized("invalid x-api-key".into()));
    assert_eq!(http.sent().len(), 1, "a refusal is not retried");
}

#[test]
fn a_cut_stream_is_an_error_and_cancelling_stops_reading() {
    let cut = include_str!("../../../../tests/fixtures/messages_tool_use.sse").replace("event: message_stop", "");
    let cut = cut.replace(r#"data: {"type":"message_stop"}"#, "");
    let http = Arc::new(ScriptedHttp::new([(200, cut.clone()), (200, cut.clone()), (200, cut.clone()), (200, cut)]));
    let error = client(&http, "k").turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect_err("cut");
    assert!(matches!(error, ModelError::Network(_)), "{error:?}");

    let stream = include_str!("../../../../tests/fixtures/messages_tool_use.sse").to_owned();
    let http = Arc::new(ScriptedHttp::new([(200, stream)]));
    let error = client(&http, "k").turn(&request(), &mut |_| {}, &AtomicBool::new(true)).expect_err("cancelled");
    assert_eq!(error, ModelError::Cancelled);
}

fn with_image(image: &domain::Image) -> TurnRequest {
    TurnRequest {
        model: "fixture".into(),
        system: "system".into(),
        messages: vec![
            Message::user("inspect"),
            Message::assistant(vec![Block::ToolUse {
                id: "read".into(),
                name: "Read".into(),
                input: json!({"file_path":"image.png"}),
            }]),
            crate::transcript::Message {
                role: crate::transcript::Role::User,
                blocks: vec![Block::ToolResultRich {
                    id: "read".into(),
                    content: "screenshot".into(),
                    images: vec![image.clone()],
                    is_error: false,
                }],
            },
        ],
        tools: vec![],
        effort: Effort::Low,
        max_tokens: 100,
    }
}

fn pixel() -> domain::Image {
    domain::Image::from_mcp(
        "image/png",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6wAAAABJRU5ErkJggg==",
    )
    .expect("valid test fixture")
}

fn thought(signature: &str, origin: Option<crate::transcript::ReasoningOrigin>) -> TurnRequest {
    TurnRequest {
        model: "fixture".into(),
        system: "system".into(),
        messages: vec![
            Message::user("question"),
            Message::assistant(vec![
                Block::Thinking { text: "private reasoning".into(), signature: Some(signature.into()), origin },
                Block::Text("Visible answer".into()),
            ]),
            Message::user("continue"),
        ],
        tools: vec![],
        effort: Effort::Low,
        max_tokens: 100,
    }
}

#[test]
fn an_image_in_a_tool_result_is_sent_visually() {
    let image = pixel();
    let body = super::body(&with_image(&image));
    assert_eq!(body["messages"][2]["content"][0]["content"][1]["source"]["data"], image.data);
}

#[test]
fn only_its_own_reasoning_signatures_are_sent_back() {
    let own = super::body(&thought("OWN-SIGNATURE", Some(super::ORIGIN))).to_string();
    assert!(own.contains("OWN-SIGNATURE") && own.contains("Visible answer"));
    let other = crate::transcript::ReasoningOrigin::of("other-wire");
    assert!(!super::body(&thought("FOREIGN", Some(other))).to_string().contains("FOREIGN"));
    assert!(!super::body(&thought("UNKNOWN", None)).to_string().contains("UNKNOWN"));
}
