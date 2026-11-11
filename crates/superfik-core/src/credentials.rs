//! What a run carries that never crosses the wire: who it acts for, the
//! provider key it calls the model with, the host's in-process MCP servers
//! and the client it belongs to. Filled by the backend for each run, so a
//! client cannot set it and logs never show it.

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RunCredentials {
    pub owner: Option<crate::Principal>,
    pub model: Option<crate::provider::ProviderKey>,
    /// The host's in-process MCP servers (the SDK's tools in code).
    pub mcp: Vec<crate::mcp::LocalMcp>,
    /// Which client connection the run belongs to (its background commands
    /// end with that client); 0 when unknown.
    pub client: u64,
    /// Access tokens for the remote MCP servers the owner signed in to
    /// (`/mcp login`), by server URL.
    pub mcp_tokens: Option<McpSignIns>,
}

/// Where a run gets a remote MCP server's access token, refreshed when
/// stale; `None` when the owner never signed in to it.
pub trait McpTokenSource: Send + Sync {
    fn token(&self, server_url: &str) -> Option<String>;
}

/// A run's [`McpTokenSource`]; runs are equal when they share one.
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
