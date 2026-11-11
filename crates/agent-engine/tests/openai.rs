//! The Responses client against recorded streams and error answers.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::openai::Responses;
use agent_engine::model::{Delta, ModelApi, ModelError, TurnRequest};
use agent_engine::transcript::{Block, Message, StopReason};
use domain::Effort;
use domain::provider::ProviderKey;
use serde_json::json;

fn request() -> TurnRequest {
    TurnRequest {
        model: "gpt-x".into(),
        system: concat!("You are ", brand::name!(), "'s agent.").into(),
        messages: vec![Message::user("Read src/main.rs")],
        tools: Vec::new(),
        effort: Effort::Medium,
        max_tokens: 64_000,
    }
}

fn key() -> Arc<ProviderKey> {
    Arc::new(ProviderKey::new("sk"))
}

#[test]
fn folds_reasoning_text_and_a_call() {
    let stream = include_str!("fixtures/openai_tool_use.sse").to_owned();
    let http = Arc::new(ScriptedHttp::new([(200, stream)]));
    let mut deltas = Vec::new();
    let reply = Responses::new(http.clone(), key())
        .turn(&request(), &mut |delta| deltas.push(delta), &AtomicBool::new(false))
        .expect("reply");
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert_eq!(
        reply.message.blocks,
        vec![
            Block::Thinking {
                text: "Look first.".into(),
                signature: Some("gAAAA".into()),
                origin: Some(agent_engine::transcript::ReasoningOrigin::OpenAi)
            },
            Block::Text("Let me look.".into()),
            Block::ToolUse { id: "call_1".into(), name: "Read".into(), input: json!({"file_path": "src/main.rs"}) },
        ]
    );
    assert_eq!(deltas[0], Delta::Thinking("Look first.".into()));
    assert_eq!(
        &deltas[1..],
        &[
            Delta::Text("Let me ".into()),
            Delta::Text("look.".into()),
            Delta::ToolInput { name: "Read".into(), piece: "{\"file_path\":\"src/main.rs\"}".into() },
        ]
    );
    assert_eq!((reply.usage.input, reply.usage.cache_read, reply.usage.output), (12, 4000, 87));
}

#[test]
fn the_key_goes_to_the_api() {
    let stream = include_str!("fixtures/openai_tool_use.sse").to_owned();
    let http = Arc::new(ScriptedHttp::new([(200, stream)]));
    Responses::new(http.clone(), key()).turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect("key");
    let sent = http.sent();
    let header = |name: &str| sent[0].headers.iter().find(|(key, _)| key == name).map(|(_, v)| v.clone());
    assert_eq!(sent[0].url, "https://api.openai.com/v1/responses");
    assert_eq!(header("authorization").as_deref(), Some("Bearer sk"));
    assert!(header("user-agent").is_some_and(|agent| agent.starts_with(concat!(brand::command!(), "/"))));
}

#[test]
fn maps_errors() {
    let http = Arc::new(ScriptedHttp::new([(401, r#"{"detail":"Unauthorized"}"#.to_owned())]));
    let error = Responses::new(http, key())
        .without_backoff()
        .turn(&request(), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("refused");
    assert!(matches!(error, ModelError::Unauthorized(_)));

    let limit = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached"}}"#;
    let http = Arc::new(ScriptedHttp::new((0..4).map(|_| (429, limit.to_owned()))));
    let error = Responses::new(http.clone(), key())
        .without_backoff()
        .turn(&request(), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("limited");
    assert_eq!(error, ModelError::Overloaded("The usage limit has been reached".into()));
    assert_eq!(http.sent().len(), 4);

    let failed = "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"context_length_exceeded\",\"message\":\"too long\"}}}\n\n";
    let http = Arc::new(ScriptedHttp::new([(200, failed.to_owned())]));
    let error = Responses::new(http, key()).turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect_err("failed");
    assert_eq!(error, ModelError::TooLong("too long".into()));
}
