//! The model providers' APIs behind one trait: a turn of conversation in,
//! streamed deltas and the assistant's message out.

pub mod anthropic;
mod cancellable_http;
pub mod chat;
pub mod http;
pub mod kiro;
pub mod openai;
pub mod providers;
#[cfg(any(test, feature = "test-support"))]
pub mod scripted;
pub mod sse;

use std::sync::atomic::AtomicBool;

use serde_json::Value;
use domain::Effort;
use domain::provider::ProviderKey;

use crate::transcript::{Message, StopReason, Usage};

#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

#[derive(Debug, Clone)]
pub struct TurnRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub effort: Effort,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    Text(String),
    Thinking(String),
    /// A piece of the JSON input of a tool call the model is writing.
    ToolInput {
        name: String,
        piece: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub message: Message,
    pub stop: StopReason,
    pub usage: Usage,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("not signed in: {0}")]
    Unauthorized(String),
    #[error("rate limited: {0}")]
    Overloaded(String),
    #[error("the conversation is too long for the model: {0}")]
    TooLong(String),
    #[error("{0}")]
    Provider(String),
    #[error("could not reach the provider: {0}")]
    Network(String),
    #[error("cancelled")]
    Cancelled,
}

impl From<llm_providers::ProviderError> for ModelError {
    fn from(error: llm_providers::ProviderError) -> Self {
        use llm_providers::ProviderError;
        match error {
            ProviderError::Unauthorized(why) => Self::Unauthorized(why),
            ProviderError::Invalid(why) => Self::Provider(why),
        }
    }
}

impl ModelError {
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Overloaded(_) | Self::Network(_))
    }
}

pub trait KeySource: Send + Sync {
    fn key(&self) -> Result<ProviderKey, ModelError>;
    fn replace(&self, _value: &str) -> Result<(), ModelError> {
        Ok(())
    }
}

impl KeySource for ProviderKey {
    fn replace(&self, value: &str) -> Result<(), ModelError> {
        ProviderKey::replace(self, value).map_err(ModelError::Provider)
    }
    fn key(&self) -> Result<ProviderKey, ModelError> {
        self.current().map_err(ModelError::Unauthorized)
    }
}

pub trait ModelApi: Send + Sync {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError>;
}
