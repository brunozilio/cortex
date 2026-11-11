use agent_engine::{
    model::{TurnRequest, anthropic, chat, openai},
    transcript::{Block, Message, Role},
};
use domain::{Effort, Image};
use serde_json::json;
#[test]
fn image_from_tool_result_is_sent_visually_to_all_wire_dialects() {
    let image = Image::from_mcp(
        "image/png",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6wAAAABJRU5ErkJggg==",
    )
    .expect("valid test fixture");
    let request = TurnRequest {
        model: "fixture".into(),
        system: "system".into(),
        messages: vec![
            Message::user("inspect"),
            Message::assistant(vec![Block::ToolUse {
                id: "read".into(),
                name: "Read".into(),
                input: json!({"file_path":"image.png"}),
            }]),
            Message {
                role: Role::User,
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
    };
    let claude = anthropic::body(&request);
    assert_eq!(claude["messages"][2]["content"][0]["content"][1]["source"]["data"], image.data);
    let openai = openai::body(&request);
    let result = openai["input"]
        .as_array()
        .expect("valid test fixture")
        .iter()
        .find(|v| v["type"] == "function_call_output")
        .expect("valid test fixture");
    assert_eq!(result["output"][1]["type"], "input_image");
    assert_eq!(result["output"][1]["image_url"], image.data_url());
    let local = chat::body(&request, "fixture");
    let visual = local["messages"]
        .as_array()
        .expect("valid test fixture")
        .iter()
        .find(|m| m["content"].is_array())
        .expect("valid test fixture");
    assert!(
        visual["content"]
            .as_array()
            .expect("valid test fixture")
            .iter()
            .any(|part| part["type"] == "image_url" && part["image_url"]["url"] == image.data_url())
    );
}

#[test]
fn switching_provider_preserves_visible_context_without_forwarding_foreign_reasoning_signatures() {
    use agent_engine::transcript::ReasoningOrigin;
    let mut request = TurnRequest {
        model: "fixture".into(),
        system: "system".into(),
        messages: vec![
            Message::user("question"),
            Message::assistant(vec![
                Block::Thinking {
                    text: "private reasoning".into(),
                    signature: Some("ANTHROPIC-SIGNATURE".into()),
                    origin: Some(ReasoningOrigin::Anthropic),
                },
                Block::Text("Visible answer".into()),
            ]),
            Message::user("continue"),
        ],
        tools: vec![],
        effort: Effort::Low,
        max_tokens: 100,
    };
    let anthropic_request = anthropic::body(&request).to_string();
    let openai_request = openai::body(&request).to_string();
    assert!(anthropic_request.contains("ANTHROPIC-SIGNATURE"));
    assert!(!openai_request.contains("ANTHROPIC-SIGNATURE"));
    assert!(openai_request.contains("Visible answer"));
    request.messages[1].blocks[0] = Block::Thinking {
        text: String::new(),
        signature: Some("OPENAI-ENCRYPTED".into()),
        origin: Some(ReasoningOrigin::OpenAi),
    };
    assert!(!anthropic::body(&request).to_string().contains("OPENAI-ENCRYPTED"));
    assert!(openai::body(&request).to_string().contains("OPENAI-ENCRYPTED"));
    let legacy: Block = serde_json::from_value(json!({"Thinking":{"text":"legacy","signature":"UNKNOWN-SIGNATURE"}}))
        .expect("legacy block");
    request.messages[1].blocks[0] = legacy;
    assert!(!anthropic::body(&request).to_string().contains("UNKNOWN-SIGNATURE"));
    assert!(!openai::body(&request).to_string().contains("UNKNOWN-SIGNATURE"));
}
