//! An in-process MCP server's transport: JSON-RPC handed to the host's
//! [`McpEndpoint`] as a function call, no HTTP and no child process.

use std::sync::Arc;

use serde_json::Value;
use superfik_core::mcp::McpEndpoint;

use super::Transport;

pub struct LocalTransport(Arc<dyn McpEndpoint>);

impl LocalTransport {
    pub fn new(endpoint: Arc<dyn McpEndpoint>) -> Self {
        Self(endpoint)
    }
}

impl std::fmt::Debug for LocalTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalTransport(..)")
    }
}

impl Transport for LocalTransport {
    fn request(&self, message: &Value) -> Result<Value, String> {
        self.0.handle(message).ok_or_else(|| "the in-process MCP server did not reply".to_owned())
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        self.0.handle(message);
        Ok(())
    }
}
