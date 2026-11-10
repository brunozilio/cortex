//! MCP servers that live in the host's process (the SDK's tools written in
//! code): the engine talks JSON-RPC to them through [`McpEndpoint`] instead
//! of HTTP or a child process. They ride on a run's
//! [`RunCredentials`](crate::credentials::RunCredentials), never on the wire.

use std::sync::Arc;

use serde_json::Value;

/// Answers one MCP JSON-RPC message: `Some(reply)` for a request, `None`
/// for a notification.
pub trait McpEndpoint: Send + Sync {
    fn handle(&self, message: &Value) -> Option<Value>;
}

/// An in-process server under the name its tools are offered as
/// (`mcp__<name>__<tool>`).
#[derive(Clone)]
pub struct LocalMcp {
    pub name: String,
    pub endpoint: Arc<dyn McpEndpoint>,
}

impl LocalMcp {
    pub fn new(name: impl Into<String>, endpoint: Arc<dyn McpEndpoint>) -> Self {
        Self { name: name.into(), endpoint }
    }
}

impl std::fmt::Debug for LocalMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalMcp").field("name", &self.name).finish_non_exhaustive()
    }
}

impl PartialEq for LocalMcp {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Arc::ptr_eq(&self.endpoint, &other.endpoint)
    }
}

impl Eq for LocalMcp {}
