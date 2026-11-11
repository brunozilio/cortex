#[derive(Clone, Default, PartialEq, Eq)]
pub struct RunCredentials {
    pub owner: Option<crate::Principal>,
    pub model: Option<crate::provider::ProviderKey>,
    pub other_models: Vec<(crate::ModelProvider, crate::provider::ProviderKey)>,
    pub mcp: Vec<crate::mcp::LocalMcp>,
    pub client: u64,
    pub mcp_tokens: Option<McpSignIns>,
}

pub trait McpTokenSource: Send + Sync {
    fn token(&self, server_url: &str) -> Option<String>;
}

#[derive(Clone)]
pub struct McpSignIns(pub std::sync::Arc<dyn McpTokenSource>);

impl PartialEq for McpSignIns {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for McpSignIns {}

impl std::fmt::Debug for McpSignIns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpSignIns")
    }
}

impl std::fmt::Debug for RunCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCredentials")
            .field("model", &self.model.is_some())
            .field("other_models", &self.other_models.iter().map(|(provider, _)| *provider).collect::<Vec<_>>())
            .field("mcp", &self.mcp)
            .field("client", &self.client)
            .field("mcp_tokens", &self.mcp_tokens.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_key_never_prints() {
        let other = (crate::ModelProvider::new("openai"), crate::provider::ProviderKey::new("sk-other-secret"));
        let credentials = RunCredentials {
            model: Some(crate::provider::ProviderKey::new("sk-ant-secret")),
            other_models: vec![other],
            ..Default::default()
        };
        let shown = format!("{credentials:?}");
        assert!(!shown.contains("secret") && shown.contains("openai"), "{shown}");
    }
}
