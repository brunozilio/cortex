//! A conversation as the agent loop keeps it, independent of any provider:
//! each provider's API module translates it to and from its own wire shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningOrigin {
    Anthropic,
    OpenAi,
    Local,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    Text(String),
    Image(domain::Image),
    ToolResultRich {
        id: String,
        content: String,
        images: Vec<domain::Image>,
        is_error: bool,
    },
    Thinking {
        text: String,
        signature: Option<String>,
        #[serde(default)]
        origin: Option<ReasoningOrigin>,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, blocks: vec![Block::Text(text.into())] }
    }

    pub fn assistant(blocks: Vec<Block>) -> Self {
        Self { role: Role::Assistant, blocks }
    }

    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &str, &Value)> {
        self.blocks.iter().filter_map(|block| match block {
            Block::ToolUse { id, name, input } => Some((id.as_str(), name.as_str(), input)),
            _ => None,
        })
    }

    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Refusal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    pub const fn prompt(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    pub fn add(&mut self, other: Usage) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_tool_calls_and_joins_text() {
        let message = Message::assistant(vec![
            Block::Text("Looking ".into()),
            Block::ToolUse { id: "t1".into(), name: "Read".into(), input: serde_json::json!({"file_path": "a"}) },
            Block::Text("now".into()),
        ]);
        assert_eq!(message.text(), "Looking now");
        let calls: Vec<_> = message.tool_uses().map(|(id, name, _)| (id, name)).collect();
        assert_eq!(calls, vec![("t1", "Read")]);
    }

    #[test]
    fn usage_adds_up() {
        let mut total = Usage { input: 1, output: 2, cache_read: 3, cache_write: 4 };
        total.add(Usage { input: 1, output: 1, cache_read: 1, cache_write: 1 });
        assert_eq!(total.prompt(), 2 + 4 + 5);
        assert_eq!(total.output, 3);
    }
}
