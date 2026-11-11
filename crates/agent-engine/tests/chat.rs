//! The Chat Completions client (local models) against recorded streams.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_engine::model::chat::{Chat, Endpoint, Endpoints};
use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::{Delta, ModelApi, ModelError, ToolSpec, TurnRequest};
use agent_engine::transcript::{Block, Message, StopReason};
use domain::Effort;
use serde_json::{Value, json};

struct Fixed(Result<Endpoint, ModelError>);

impl Endpoints for Fixed {
    fn endpoint(&self, _model: &str) -> Result<Endpoint, ModelError> {
        self.0.clone()
    }
}

fn served() -> Arc<Fixed> {
    Arc::new(Fixed(Ok(Endpoint { base_url: "http://127.0.0.1:8080".into(), model: "/models/qwen".into() })))
}

fn request() -> TurnRequest {
    TurnRequest {
        model: "mlx-community/Qwen3-8B".into(),
        system: concat!("You are ", brand::name!(), "'s agent.").into(),
        messages: vec![Message::user("Read src/main.rs")],
        tools: vec![ToolSpec {
            name: "Read".into(),
            description: "Reads a file.".into(),
            schema: json!({"type": "object", "properties": {"file_path": {"type": "string"}}}),
        }],
        effort: Effort::Medium,
        max_tokens: 4_096,
    }
}

#[test]
fn folds_thinking_text_and_a_call_from_a_streamed_answer() {
    let http = Arc::new(ScriptedHttp::new([(200, include_str!("fixtures/chat_tool_use.sse").to_owned())]));
    let mut deltas = Vec::new();
    let reply = Chat::new(http.clone(), served())
        .turn(&request(), &mut |delta| deltas.push(delta), &AtomicBool::new(false))
        .expect("reply");
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert_eq!(
        reply.message.blocks,
        vec![
            Block::Thinking {
                text: "Read it first.".into(),
                signature: None,
                origin: Some(agent_engine::transcript::ReasoningOrigin::Local)
            },
            Block::Text("Let me look.".into()),
            Block::ToolUse { id: "call_1".into(), name: "Read".into(), input: json!({"file_path": "src/main.rs"}) },
        ]
    );
    let thinking: String =
        deltas.iter().filter_map(|d| if let Delta::Thinking(t) = d { Some(t.as_str()) } else { None }).collect();
    let text: String =
        deltas.iter().filter_map(|d| if let Delta::Text(t) = d { Some(t.as_str()) } else { None }).collect();
    assert_eq!((thinking.as_str(), text.as_str()), ("Read it first.", "Let me look."));
    assert_eq!((reply.usage.input, reply.usage.output), (120, 40));
}

#[test]
fn folds_a_recorded_chat_answer_with_reasoning_and_a_tool_call() {
    let stream = include_str!("fixtures/chat_reasoning_tool_call.sse").to_owned();
    let http = Arc::new(ScriptedHttp::new([(200, stream)]));
    let reply = Chat::new(http, served()).turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect("reply");
    assert_eq!(reply.stop, StopReason::ToolUse);
    assert_eq!(
        reply.message.blocks,
        vec![
            Block::Thinking {
                text: "The user is asking me to list the files in the current directory. Let's run the ls command."
                    .into(),
                signature: None,
                origin: Some(agent_engine::transcript::ReasoningOrigin::Local)
            },
            Block::ToolUse {
                id: "6a0e1e76-3e23-43de-b714-0892ec3343a0".into(),
                name: "Bash".into(),
                input: json!({"command": "ls -la"}),
            },
        ]
    );
    assert_eq!((reply.usage.input, reply.usage.output), (324, 52));
}

#[test]
fn asks_the_served_model_with_the_whole_conversation_and_the_tools() {
    let http = Arc::new(ScriptedHttp::new([(200, include_str!("fixtures/chat_tool_use.sse").to_owned())]));
    let mut request = request();
    request.messages.push(Message::assistant(vec![
        Block::Thinking {
            text: "hm".into(),
            signature: None,
            origin: Some(agent_engine::transcript::ReasoningOrigin::Local),
        },
        Block::Text("Reading.".into()),
        Block::ToolUse { id: "call_0".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
    ]));
    request.messages.push(agent_engine::transcript::Message {
        role: agent_engine::transcript::Role::User,
        blocks: vec![Block::ToolResult { id: "call_0".into(), content: "no such file".into(), is_error: true }],
    });
    Chat::new(http.clone(), served()).turn(&request, &mut |_| {}, &AtomicBool::new(false)).expect("reply");
    let sent = http.sent();
    assert_eq!(sent[0].url, "http://127.0.0.1:8080/v1/chat/completions");
    assert!(sent[0].headers.iter().all(|(name, _)| name != "authorization"), "a local server takes no key");
    let body: Value = serde_json::from_str(&sent[0].body).expect("json");
    assert_eq!(body["model"], "/models/qwen");
    assert_eq!((body["stream"].clone(), body["stream_options"]["include_usage"].clone()), (json!(true), json!(true)));
    assert_eq!(body["max_tokens"], 4_096);
    assert_eq!(
        body["messages"],
        json!([
            {"role": "system", "content": concat!("You are ", brand::name!(), "'s agent.")},
            {"role": "user", "content": "Read src/main.rs"},
            {"role": "assistant", "content": "Reading.", "tool_calls": [
                {"id": "call_0", "type": "function", "function": {"name": "Read", "arguments": "{\"file_path\":\"a\"}"}}
            ]},
            {"role": "tool", "tool_call_id": "call_0", "content": "Error: no such file"},
        ])
    );
    assert_eq!(body["tools"][0]["function"]["name"], "Read");
    assert_eq!(body["tools"][0]["function"]["parameters"]["properties"]["file_path"]["type"], "string");
}

#[test]
fn a_model_still_loading_is_busy_and_a_refused_request_says_why() {
    let loading = Arc::new(Fixed(Err(ModelError::Overloaded("qwen is still starting".into()))));
    let http = Arc::new(ScriptedHttp::new([]));
    let error = Chat::new(http.clone(), loading)
        .without_backoff()
        .turn(&request(), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("loading");
    assert_eq!(error, ModelError::Overloaded("qwen is still starting".into()));
    assert!(http.sent().is_empty());

    let refused = r#"{"error":{"message":"context length exceeded"}}"#;
    let http = Arc::new(ScriptedHttp::new([(400, refused.to_owned())]));
    let error = Chat::new(http, served()).turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect_err("refused");
    assert_eq!(error, ModelError::Provider("context length exceeded".into()));
}

#[test]
fn an_answer_cut_off_by_its_length_says_so() {
    let stream = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Half\"},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"length\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let http = Arc::new(ScriptedHttp::new([(200, stream.to_owned())]));
    let reply = Chat::new(http, served()).turn(&request(), &mut |_| {}, &AtomicBool::new(false)).expect("reply");
    assert_eq!((reply.stop, reply.message.text()), (StopReason::MaxTokens, "Half".to_owned()));
}
