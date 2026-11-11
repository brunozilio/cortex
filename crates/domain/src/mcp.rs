use std::sync::Arc;

use serde_json::{Value, json};

pub trait McpEndpoint: Send + Sync {
    fn handle(&self, message: &Value) -> Option<Value>;

    fn handle_stopped(&self, message: &Value, _stop: &std::sync::atomic::AtomicBool) -> Option<Value> {
        self.handle(message)
    }
}

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub type RpcError = (i64, String);

pub fn respond(
    message: &Value,
    server: &str,
    tools: impl FnOnce() -> Vec<Value>,
    call: impl FnOnce(&str, &Value) -> Result<Value, RpcError>,
) -> Option<Value> {
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": server, "version": env!("CARGO_PKG_VERSION") },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            call(name, &arguments)
        }
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
}

pub fn text_result(text: impl Into<String>, error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": error })
}

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
