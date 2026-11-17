use std::sync::Arc;

use domain::mcp::McpEndpoint;
use serde_json::Value;

use super::channel::Bridge;
use super::{McpError, Transport};

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
    fn request(&self, message: &Value) -> Result<Value, McpError> {
        self.0.handle(message).ok_or_else(|| "the in-process MCP server did not reply".into())
    }

    fn request_stopped(&self, message: &Value, stop: &platform::Stop) -> Result<Value, McpError> {
        if stop.is_stopped() {
            return Err(McpError::Cancelled);
        }
        let endpoint: &dyn std::any::Any = &*self.0;
        let answer = match endpoint.downcast_ref::<Bridge>() {
            Some(bridge) => bridge.answer(message, stop),
            None => self.0.handle_stopped(message, stop),
        };
        answer.ok_or_else(|| "the in-process MCP server did not reply".into())
    }

    fn notify(&self, message: &Value) -> Result<(), McpError> {
        self.0.handle(message);
        Ok(())
    }
}
