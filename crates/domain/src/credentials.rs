#[derive(Clone, Default, PartialEq, Eq)]
pub struct RunCredentials {
    pub owner: Option<crate::Principal>,
    pub model: Option<crate::provider::ProviderKey>,
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
        let credentials =
            RunCredentials { model: Some(crate::provider::ProviderKey::new("sk-ant-secret")), ..Default::default() };
        assert!(!format!("{credentials:?}").contains("secret"));
    }
}
