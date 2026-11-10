//! What a backend says about LLM providers.

use serde::{Deserialize, Serialize};
use superfik_core::provider::{ModelProvider, ProviderSummary};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderEvent {
    Listed { providers: Vec<ProviderSummary> },
    AuthorizeAt { provider: ModelProvider, url: String },
    Failed { provider: ModelProvider, message: String },
}
