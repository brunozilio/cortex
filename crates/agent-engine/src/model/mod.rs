pub mod anthropic;
mod cancellable_http;
pub mod chat;
mod conversation_key;
pub mod event_stream;
pub mod http;
pub mod openai;
pub mod providers;
mod retry;
pub mod routed;
#[cfg(any(test, feature = "test-support"))]
pub mod scripted;
pub mod sse;
mod sse_read;

use std::sync::atomic::AtomicBool;

use domain::Effort;
use domain::provider::ProviderKey;
use serde_json::Value;

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
    ToolInput { name: String, piece: String },
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

    pub const fn failure(&self) -> Option<domain::fallback::Failure> {
        use domain::fallback::Failure;
        match self {
            Self::Unauthorized(_) => Some(Failure::Unauthorized),
            Self::Overloaded(_) => Some(Failure::Overloaded),
            Self::TooLong(_) => Some(Failure::TooLong),
            Self::Provider(_) => Some(Failure::Provider),
            Self::Network(_) => Some(Failure::Network),
            Self::Cancelled => None,
        }
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
