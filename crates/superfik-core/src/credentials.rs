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
}

impl std::fmt::Debug for RunCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCredentials")
            .field("model", &self.model.is_some())
            .field("mcp", &self.mcp)
            .field("client", &self.client)
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
