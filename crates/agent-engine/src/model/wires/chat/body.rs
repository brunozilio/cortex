use serde_json::{Value, json};

use crate::model::TurnRequest;
use crate::transcript::{Block, Message, Role};

pub fn body(request: &TurnRequest, model: &str) -> Value {
    let mut messages = Vec::new();
    if !request.system.is_empty() {
        messages.push(json!({ "role": "system", "content": request.system }));
    }
    for message in &request.messages {
        match message.role {
            Role::User => user(message, &mut messages),
            Role::Assistant => messages.push(assistant(message)),
        }
    }
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": request.max_tokens,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": { "name": tool.name, "description": tool.description, "parameters": tool.schema },
                })
            })
            .collect();
    }
    body
}

fn user(message: &Message, messages: &mut Vec<Value>) {
    let mut text = String::new();
    let mut images = Vec::new();
    for block in &message.blocks {
        match block {
            Block::ToolResult { id, content, is_error } => {
                let content = if *is_error { format!("Error: {content}") } else { content.clone() };
                messages.push(json!({ "role": "tool", "tool_call_id": id, "content": content }));
            }
            Block::Text(part) => text.push_str(part),
            Block::Reminder(_) => {
                let sent = block.sent_text().unwrap_or_default();
                text.push_str(&if text.is_empty() { sent } else { format!("\n\n{sent}") });
            }
            Block::Image(image) => images.push(image.clone()),
            Block::ToolResultRich { id, content, images: found, is_error } => {
                messages.push(json!({"role":"tool", "tool_call_id":id, "content":if *is_error {format!("Error: {content}")} else {content.clone()}}));
                images.extend(found.iter().cloned());
            }
            Block::Thinking { .. } | Block::ToolUse { .. } => {}
        }
    }
    if !images.is_empty() {
        let mut parts = Vec::new();
        if !text.is_empty() {
            parts.push(json!({"type":"text", "text":text}));
        }
        parts.extend(images.iter().map(|image| json!({"type":"image_url", "image_url":{"url":image.data_url()}})));
        messages.push(json!({"role":"user", "content":parts}));
    } else if !text.is_empty() {
        messages.push(json!({ "role": "user", "content": text }));
    }
}

fn assistant(message: &Message) -> Value {
    let calls: Vec<Value> = message
        .tool_uses()
        .map(|(id, name, input)| {
            let arguments = if input.is_null() { "{}".to_owned() } else { input.to_string() };
            json!({ "id": id, "type": "function", "function": { "name": name, "arguments": arguments } })
        })
        .collect();
    let text = message.text();
    let mut turn = json!({ "role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text) } });
    if !calls.is_empty() {
        turn["tool_calls"] = Value::Array(calls);
    }
    turn
}
