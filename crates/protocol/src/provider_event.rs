//! What a backend says about LLM providers.

use domain::provider::{ModelProvider, ProviderSummary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderEvent {
    Listed {
        providers: Vec<ProviderSummary>,
    },
    /// Where the user signs in; `code` is the one to type there when the
    /// sign-in is a device flow (GitHub Copilot).
    AuthorizeAt {
        provider: ModelProvider,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    Failed {
        provider: ModelProvider,
        message: String,
    },
}
