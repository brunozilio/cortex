mod body;
mod cancellable_http;
pub mod client;
pub(crate) mod conversation_key;
pub mod http;
pub mod pooled;
pub(crate) mod retry;
pub mod routed;
#[cfg(any(test, feature = "test-support"))]
pub mod scripted;
#[cfg(any(test, feature = "test-support"))]
mod scripted_http;
pub mod sse;
pub(crate) mod sse_read;
pub mod web_client;
pub mod wires;

use domain::Effort;
use domain::provider::ProviderKey;
use serde_json::Value;

use crate::transcript::{Message, StopReason, Usage};

pub use tokio_util::sync::CancellationToken;

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub type OnDelta<'a> = &'a mut (dyn FnMut(Delta) + Send);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TurnRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub effort: Effort,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Delta {
    Text(String),
    Thinking(String),
    ToolInput { name: String, piece: String },
    Searching { id: String, query: String },
    Searched { id: String, found: String },
    HandedOver { from: String, to: String },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reply {
    pub message: Message,
    pub stop: StopReason,
    pub usage: Usage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served: Option<String>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    #[error("the provider sent nothing for {0} seconds")]
    Stalled(u64),
    #[error("the provider sent a line or event longer than {0} bytes")]
    Oversized(usize),
    #[error("cancelled")]
    Cancelled,
}

impl From<llm_providers::ProviderError> for ModelError {
    fn from(error: llm_providers::ProviderError) -> Self {
        use llm_providers::ProviderError;
        match error {
            ProviderError::Unauthorized(why) => Self::Unauthorized(why),
            ProviderError::Invalid(why) => Self::Provider(why),
            taken @ ProviderError::PortInUse(_) => Self::Provider(taken.to_string()),
        }
    }
}

impl From<&ModelError> for domain::FailureKind {
    fn from(error: &ModelError) -> Self {
        match error {
            ModelError::Overloaded(_) => Self::Unavailable,
            _ => Self::Other,
        }
    }
}

impl From<ModelError> for domain::failure::RunFailure {
    fn from(error: ModelError) -> Self {
        Self { kind: domain::FailureKind::from(&error), message: error.to_string() }
    }
}

impl ModelError {
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Overloaded(_) | Self::Network(_) | Self::Stalled(_))
    }

    pub const fn failure(&self) -> Option<domain::fallback::Failure> {
        use domain::fallback::Failure;
        match self {
            Self::Unauthorized(_) => Some(Failure::Unauthorized),
            Self::Overloaded(_) => Some(Failure::Overloaded),
            Self::TooLong(_) => Some(Failure::TooLong),
            Self::Provider(_) | Self::Oversized(_) => Some(Failure::Provider),
            Self::Network(_) | Self::Stalled(_) => Some(Failure::Network),
            Self::Cancelled => None,
        }
    }
}

pub trait KeySource: Send + Sync {
    fn key(&self) -> Result<ProviderKey, ModelError>;
    fn replace(&self, _value: &str) -> Result<(), ModelError> {
        Ok(())
    }
    fn hold_for_refresh(&self) -> Result<Option<domain::provider::RefreshHold>, ModelError> {
        Ok(None)
    }
}

impl KeySource for ProviderKey {
    fn replace(&self, value: &str) -> Result<(), ModelError> {
        ProviderKey::replace(self, value).map_err(ModelError::Provider)
    }
    fn hold_for_refresh(&self) -> Result<Option<domain::provider::RefreshHold>, ModelError> {
        ProviderKey::hold_for_refresh(self).map_err(ModelError::Network)
    }
    fn key(&self) -> Result<ProviderKey, ModelError> {
        self.current().map_err(ModelError::Unauthorized)
    }
}

pub trait ModelApi: Send + Sync {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>>;
}

pub trait BlockingTurn {
    fn turn_blocking(
        &self,
        request: &TurnRequest,
        on: &mut (dyn FnMut(Delta) + Send),
        cancel: &CancellationToken,
    ) -> Result<Reply, ModelError>;
}

impl<T: ModelApi + ?Sized> BlockingTurn for T {
    fn turn_blocking(
        &self,
        request: &TurnRequest,
        on: &mut (dyn FnMut(Delta) + Send),
        cancel: &CancellationToken,
    ) -> Result<Reply, ModelError> {
        platform::runtime::block_on(self.turn(request, on, cancel))
            .unwrap_or_else(|error| Err(ModelError::Network(error.to_string())))
    }
}

pub type Searched = Option<Result<Vec<crate::web_search::Hit>, ModelError>>;

pub trait Searcher: Send + Sync {
    fn search<'a>(&'a self, query: &'a str, cancel: &'a CancellationToken) -> BoxFuture<'a, Searched>;
}

impl Searcher for client::ProviderClient {
    fn search<'a>(&'a self, query: &'a str, cancel: &'a CancellationToken) -> BoxFuture<'a, Searched> {
        Box::pin(client::ProviderClient::search(self, query, cancel))
    }
}
