//! What the app's own MCP servers share: enough JSON-RPC for tools
//! (streamable HTTP, JSON responses only) and the loopback socket's
//! bearer tokens and replies. [`respond`] is pure.

use serde_json::{Value, json};

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

pub fn text(text: impl Into<String>, error: bool) -> Value {
    let mut result = json!({ "content": [{ "type": "text", "text": text.into() }] });
    if error {
        result["isError"] = Value::Bool(true);
    }
    result
}

pub fn unknown_tool(name: &str) -> RpcError {
    (-32602, format!("unknown tool: {name}"))
}

pub fn bearer(request: &tiny_http::Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(str::to_owned))
}

pub fn read_message(request: &mut tiny_http::Request) -> Result<Value, u16> {
    if *request.method() != tiny_http::Method::Post {
        return Err(405);
    }
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return Err(400);
    }
    serde_json::from_str(&body).map_err(|_| 400)
}

pub fn reply(request: tiny_http::Request, status: u16, body: Option<Value>) {
    let response = match body {
        Some(body) => {
            let header = tiny_http::Header::from_bytes("Content-Type", "application/json").ok();
            let mut response = tiny_http::Response::from_string(body.to_string()).with_status_code(status);
            if let Some(header) = header {
                response.add_header(header);
            }
            response
        }
        None => tiny_http::Response::from_string(String::new()).with_status_code(status),
    };
    if let Err(error) = request.respond(response) {
        tracing::debug!(%error, "tool client went away");
    }
}
