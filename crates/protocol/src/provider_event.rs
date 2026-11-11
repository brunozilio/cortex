use domain::provider::{ModelProvider, ProviderSummary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderEvent {
    Listed {
        providers: Vec<ProviderSummary>,
    },
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
