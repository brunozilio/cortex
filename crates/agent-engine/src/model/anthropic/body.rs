use domain::Effort;
use serde_json::{Value, json};

use crate::model::TurnRequest;
use crate::transcript::{Block, Message, Role};

pub fn body(request: &TurnRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
        "stream": true,
        "thinking": { "type": "adaptive", "display": "summarized" },
        "output_config": { "effort": effort(request.effort) },
        "cache_control": { "type": "ephemeral" },
        "system": [{ "type": "text", "text": request.system, "cache_control": { "type": "ephemeral" } }],
        "messages": request.messages.iter().filter_map(message).collect::<Vec<_>>(),
    });
    let mut tools: Vec<Value> = request
        .tools
        .iter()
        .map(|tool| json!({ "name": tool.name, "description": tool.description, "input_schema": tool.schema }))
        .collect();
    if let Some(last) = tools.last_mut() {
        last["cache_control"] = json!({ "type": "ephemeral" });
        body["tools"] = Value::Array(tools);
    }
    body
}

pub const fn effort(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
        Effort::XHigh => "xhigh",
        Effort::Max => "max",
    }
}

fn message(message: &Message) -> Option<Value> {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    let content: Vec<_> = message
        .blocks
        .iter()
        .filter(|block| match block {
            Block::Thinking { origin, signature, .. } => {
                *origin == Some(crate::transcript::ReasoningOrigin::Anthropic) && signature.is_some()
            }
            _ => true,
        })
        .map(block)
        .collect();
    (!content.is_empty()).then(|| json!({"role":role,"content":content}))
}

fn block(block: &Block) -> Value {
    match block {
        Block::Text(text) => json!({ "type": "text", "text": text }),
        Block::Image(image) => image_block(image),
        Block::ToolResultRich { id, content, images, is_error } => {
            let mut parts = vec![json!({"type":"text", "text":content})];
            parts.extend(images.iter().map(image_block));
            json!({"type":"tool_result", "tool_use_id":id, "content":parts, "is_error":is_error})
        }
        Block::Thinking { text, signature, .. } => {
            json!({ "type": "thinking", "thinking": text, "signature": signature.clone().unwrap_or_default() })
        }
        Block::ToolUse { id, name, input } => json!({ "type": "tool_use", "id": id, "name": name, "input": input }),
        Block::ToolResult { id, content, is_error } => {
            json!({ "type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error })
        }
    }
}

fn image_block(image: &domain::Image) -> Value {
    json!({"type":"image", "source":{"type":"base64", "media_type":image.media_type, "data":image.data}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ToolSpec;

    fn request() -> TurnRequest {
        TurnRequest {
            model: "claude-opus-5-5".into(),
            system: "Be brief.".into(),
            messages: vec![
                Message::user("Read a"),
                Message::assistant(vec![
                    Block::Thinking {
                        text: "plan".into(),
                        signature: Some("sig".into()),
                        origin: Some(crate::transcript::ReasoningOrigin::Anthropic),
                    },
                    Block::ToolUse { id: "t1".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
                ]),
                Message {
                    role: Role::User,
                    blocks: vec![Block::ToolResult { id: "t1".into(), content: "x".into(), is_error: false }],
                },
            ],
            tools: vec![ToolSpec {
                name: "Read".into(),
                description: "Reads".into(),
                schema: json!({"type": "object"}),
            }],
            effort: Effort::XHigh,
            max_tokens: 64_000,
        }
    }

    #[test]
    fn streams_with_adaptive_thinking_effort_and_caching() {
        let body = body(&request());
        assert_eq!(body["stream"], true);
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "xhigh");
        assert_eq!(body["cache_control"]["type"], "ephemeral");
        assert_eq!(body["system"][0]["text"], "Be brief.");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral", "tools and system cached on their own");
        assert!(body.get("temperature").is_none(), "sampling settings are refused by the API");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn sends_thinking_back_signed_and_pairs_tool_results() {
        let body = body(&request());
        let assistant = &body["messages"][1]["content"];
        assert_eq!(assistant[0], json!({"type": "thinking", "thinking": "plan", "signature": "sig"}));
        assert_eq!(assistant[1]["type"], "tool_use");
        let result = &body["messages"][2]["content"][0];
        assert_eq!(result["tool_use_id"], "t1");
        assert_eq!(result["is_error"], false);
    }

    #[test]
    fn the_last_tool_ends_its_own_cached_prefix() {
        let mut request = request();
        request.tools.push(ToolSpec { name: "Grep".into(), description: "Greps".into(), schema: json!({}) });
        let body = body(&request);
        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(body["tools"][1]["cache_control"]["type"], "ephemeral", "tools stay cached when the system changes");
    }

    #[test]
    fn leaves_tools_out_when_there_are_none() {
        let mut request = request();
        request.tools.clear();
        assert!(body(&request).get("tools").is_none());
    }
}
