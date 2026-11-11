use domain::Effort;
use serde_json::{Value, json};

use crate::model::TurnRequest;
use crate::transcript::{Block, Message, Role};

pub fn body(request: &TurnRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "instructions": request.system,
        "input": request.messages.iter().flat_map(items).collect::<Vec<_>>(),
        "reasoning": { "effort": effort(request.effort), "summary": "auto" },
        "store": false,
        "stream": true,
        "prompt_cache_key": crate::model::conversation_key::conversation_key(request).to_string(),
        "include": ["reasoning.encrypted_content"],
    });
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.schema,
                    "strict": false,
                })
            })
            .collect();
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(true);
    }
    body
}

pub const fn effort(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High | Effort::XHigh | Effort::Max => "high",
    }
}

fn items(message: &Message) -> Vec<Value> {
    let mut items = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, items: &mut Vec<Value>| {
        if text.is_empty() {
            return;
        }
        let (role, kind) = match message.role {
            Role::User => ("user", "input_text"),
            Role::Assistant => ("assistant", "output_text"),
        };
        items.push(
            json!({ "type": "message", "role": role, "content": [{ "type": kind, "text": std::mem::take(text) }] }),
        );
    };
    for block in &message.blocks {
        match block {
            Block::Text(part) => text.push_str(part),
            Block::Reminder(_) => {
                let sent = block.sent_text().unwrap_or_default();
                text.push_str(&if text.is_empty() { sent } else { format!("\n\n{sent}") });
            }
            Block::Image(image) => {
                flush(&mut text, &mut items);
                items.push(json!({"type":"message", "role":"user", "content":[{"type":"input_image", "image_url":image.data_url(), "detail":"auto"}]}));
            }
            Block::ToolResultRich { id, content, images, is_error } => {
                flush(&mut text, &mut items);
                let mut output = vec![
                    json!({"type":"input_text", "text":if *is_error {format!("Error: {content}")} else {content.clone()}}),
                ];
                output.extend(
                    images
                        .iter()
                        .map(|image| json!({"type":"input_image", "image_url":image.data_url(), "detail":"auto"})),
                );
                items.push(json!({"type":"function_call_output", "call_id":id, "output":output}));
            }
            Block::Thinking {
                signature: Some(encrypted),
                origin: Some(crate::transcript::ReasoningOrigin::OpenAi),
                ..
            } => {
                flush(&mut text, &mut items);
                items.push(json!({ "type": "reasoning", "summary": [], "encrypted_content": encrypted }));
            }
            Block::Thinking { .. } => {}
            Block::ToolUse { id, name, input } => {
                flush(&mut text, &mut items);
                items.push(
                    json!({ "type": "function_call", "call_id": id, "name": name, "arguments": input.to_string() }),
                );
            }
            Block::ToolResult { id, content, is_error } => {
                flush(&mut text, &mut items);
                let output = if *is_error { format!("Error: {content}") } else { content.clone() };
                items.push(json!({ "type": "function_call_output", "call_id": id, "output": output }));
            }
        }
    }
    flush(&mut text, &mut items);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ToolSpec;

    #[test]
    fn translates_the_transcript_into_items() {
        let request = TurnRequest {
            model: "gpt-x".into(),
            system: "Be brief.".into(),
            messages: vec![
                Message::user("Read a"),
                Message::assistant(vec![
                    Block::Thinking {
                        text: "plan".into(),
                        signature: Some("enc".into()),
                        origin: Some(crate::transcript::ReasoningOrigin::OpenAi),
                    },
                    Block::Text("Reading.".into()),
                    Block::ToolUse { id: "c1".into(), name: "Read".into(), input: json!({"file_path": "a"}) },
                ]),
                Message {
                    role: Role::User,
                    blocks: vec![Block::ToolResult { id: "c1".into(), content: "gone".into(), is_error: true }],
                },
            ],
            tools: vec![ToolSpec {
                name: "Read".into(),
                description: "Reads".into(),
                schema: json!({"type": "object"}),
            }],
            effort: Effort::Max,
            max_tokens: 1000,
        };
        let body = body(&request);
        assert_eq!(body["instructions"], "Be brief.");
        assert_eq!(body["store"], false);
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["tools"][0]["parameters"]["type"], "object");
        let input = body["input"].as_array().expect("input");
        assert_eq!(
            input[0],
            json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Read a"}]})
        );
        assert_eq!(input[1]["encrypted_content"], "enc");
        assert_eq!(input[2]["content"][0]["type"], "output_text");
        assert_eq!(
            input[3],
            json!({"type": "function_call", "call_id": "c1", "name": "Read", "arguments": "{\"file_path\":\"a\"}"})
        );
        assert_eq!(input[4], json!({"type": "function_call_output", "call_id": "c1", "output": "Error: gone"}));
        assert!(body.get("max_output_tokens").is_none(), "the ChatGPT backend refuses it");
        let key = crate::model::conversation_key::conversation_key(&request).to_string();
        assert_eq!(body["prompt_cache_key"], key.as_str(), "turns of one conversation share a cache");
    }
}
