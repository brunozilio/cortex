//! What Claude Code sends: Anthropic's Messages API, read leniently. Only
//! what a translation needs is typed; anything else is ignored, so a newer
//! CLI adding fields still parses.

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    #[serde(default)]
    pub system: Option<System>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<Tool>,
    #[serde(default)]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum System {
    Text(String),
    Blocks(Vec<Block>),
}

impl System {
    pub fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Blocks(blocks) => join_text(blocks),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Content,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

impl Content {
    pub fn blocks(&self) -> Vec<Block> {
        match self {
            Self::Text(text) => vec![Block::Text { text: text.clone() }],
            Self::Blocks(blocks) => blocks.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Option<Content>,
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
    RedactedThinking {},
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

impl ImageSource {
    pub fn url(&self) -> String {
        match self {
            Self::Base64 { media_type, data } => format!("data:{media_type};base64,{data}"),
            Self::Url { url } => url.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    Any,
    None,
    Tool { name: String },
}

pub fn join_text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
