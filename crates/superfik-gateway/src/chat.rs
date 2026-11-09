//! Messages → Chat Completions, the API local servers (`mlx_lm.server`)
//! speak. Pure.

use serde::Serialize;
use serde_json::{Value, json};

use crate::messages::{self, Block, MessagesRequest, Role, ToolChoice};

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: UserContent,
    },
    Assistant {
        content: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<Value>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Parts(Vec<Value>),
}

pub fn request(request: &MessagesRequest) -> ChatRequest {
    let mut messages = Vec::new();
    if let Some(system) = request.system.as_ref().map(messages::System::text).filter(|s| !s.is_empty()) {
        messages.push(ChatMessage::System { content: system });
    }
    for message in &request.messages {
        match (message.role, &message.content) {
            (Role::User, messages::Content::Text(text)) => {
                push_user(&mut messages, UserContent::Text(text.clone()));
            }
            (Role::User | Role::System, content) => user_turn(&mut messages, &content.blocks()),
            (Role::Assistant, content) => messages.push(assistant_turn(&content.blocks())),
        }
    }
    ChatRequest {
        model: request.model.clone(),
        messages,
        max_tokens: request.max_tokens,
        tools: request.tools.iter().map(tool).collect(),
        tool_choice: request.tool_choice.as_ref().map(tool_choice),
        temperature: request.temperature,
        top_p: request.top_p,
        stop: request.stop_sequences.clone(),
        stream: request.stream,
        stream_options: request.stream.then(|| json!({ "include_usage": true })),
    }
}

fn user_turn(messages: &mut Vec<ChatMessage>, blocks: &[Block]) {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            Block::ToolResult { tool_use_id, content, is_error } => {
                let text = content.as_ref().map(|c| messages::join_text(&c.blocks())).unwrap_or_default();
                let content = if *is_error { format!("Error: {text}") } else { text };
                messages.push(ChatMessage::Tool { tool_call_id: tool_use_id.clone(), content });
            }
            Block::Text { text } => parts.push(json!({ "type": "text", "text": text })),
            Block::Image { source } => parts.push(json!({ "type": "image_url", "image_url": { "url": source.url() } })),
            _ => {}
        }
    }
    if !parts.is_empty() {
        push_user(messages, UserContent::Parts(parts));
    }
}

fn push_user(messages: &mut Vec<ChatMessage>, content: UserContent) {
    if let Some(ChatMessage::User { content: previous }) = messages.last_mut() {
        let mut parts = into_parts(std::mem::replace(previous, UserContent::Parts(Vec::new())));
        parts.extend(into_parts(content));
        *previous = UserContent::Parts(parts);
        return;
    }
    messages.push(ChatMessage::User { content });
}

fn into_parts(content: UserContent) -> Vec<Value> {
    match content {
        UserContent::Text(text) => vec![json!({ "type": "text", "text": text })],
        UserContent::Parts(parts) => parts,
    }
}

fn assistant_turn(blocks: &[Block]) -> ChatMessage {
    let text = messages::join_text(blocks);
    let tool_calls = blocks
        .iter()
        .filter_map(|block| match block {
            Block::ToolUse { id, name, input } => Some(json!({
                "id": id,
                "type": "function",
                "function": { "name": name, "arguments": arguments(input) },
            })),
            _ => None,
        })
        .collect();
    ChatMessage::Assistant { content: (!text.is_empty()).then_some(text), tool_calls }
}

fn arguments(input: &Value) -> String {
    match input {
        Value::Null => "{}".to_owned(),
        input => input.to_string(),
    }
}

fn tool(tool: &messages::Tool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description.clone().unwrap_or_default(),
            "parameters": tool.input_schema,
        },
    })
}

fn tool_choice(choice: &ToolChoice) -> Value {
    match choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::Any => json!("required"),
        ToolChoice::None => json!("none"),
        ToolChoice::Tool { name } => json!({ "type": "function", "function": { "name": name } }),
    }
}
