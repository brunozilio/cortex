//! What a backend says about model providers.

use serde::{Deserialize, Serialize};
use superfik_core::provider::{ModelProvider, ProviderSummary};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderEvent {
    Listed { providers: Vec<ProviderSummary> },
    Download { id: String, state: DownloadState },
    Failed { provider: ModelProvider, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum DownloadState {
    Started,
    Preparing,
    Fetching { done: u64, total: Option<u64> },
    Done,
    Failed(String),
}

impl DownloadState {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Started | Self::Preparing | Self::Fetching { .. })
    }
}
