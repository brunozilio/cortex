//! MCP for the host program: tools written in code ([`McpServer`], served
//! in process, no HTTP) and servers reached over HTTP or stdio
//! ([`McpConnection`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use superfik_backend::McpServerSpec;
use superfik_core::mcp::McpEndpoint;

type Handler = dyn Fn(Value) -> Result<Value, String> + Send + Sync;

struct McpTool {
    name: String,
    description: String,
    schema: Value,
    handler: Arc<Handler>,
}

/// An MCP server whose tools are Rust closures. The agent sees each tool
/// as `mcp__<server>__<tool>`; a tool returns a string (sent as text) or
/// any JSON value (sent as its JSON text), or an error message.
///
/// ```
/// use superfik_sdk::McpServer;
/// use serde_json::json;
///
/// let math = McpServer::new("math").tool(
///     "add",
///     "Adds two numbers.",
///     json!({"type": "object", "properties": {"a": {"type": "number"}, "b": {"type": "number"}}}),
///     |input| Ok(json!(input["a"].as_f64().unwrap_or(0.0) + input["b"].as_f64().unwrap_or(0.0))),
/// );
/// assert_eq!(math.name(), "math");
/// ```
#[derive(Clone)]
pub struct McpServer {
    name: String,
    tools: Vec<Arc<McpTool>>,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tools: Vec<&str> = self.tools.iter().map(|tool| tool.name.as_str()).collect();
        f.debug_struct("McpServer").field("name", &self.name).field("tools", &tools).finish()
    }
}

impl McpServer {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), tools: Vec::new() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Adds a tool; `schema` is its input's JSON Schema.
    pub fn tool(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        schema: Value,
        handler: impl Fn(Value) -> Result<Value, String> + Send + Sync + 'static,
    ) -> Self {
        self.tools.push(Arc::new(McpTool {
            name: name.into(),
            description: description.into(),
            schema,
            handler: Arc::new(handler),
        }));
        self
    }

    fn call(&self, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or_default();
        let Some(tool) = self.tools.iter().find(|tool| tool.name == name) else {
            return text(format!("no tool named {name}"), true);
        };
        let input = if params["arguments"].is_null() { json!({}) } else { params["arguments"].clone() };
        match (tool.handler)(input) {
            Ok(Value::String(output)) => text(output, false),
            Ok(output) => text(output.to_string(), false),
            Err(message) => text(message, true),
        }
    }
}

fn text(text: String, failed: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": failed })
}

impl McpEndpoint for McpServer {
    fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id")?.clone();
        let result = match message["method"].as_str() {
            Some("initialize") => json!({
                "protocolVersion": message["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": self.name, "version": env!("CARGO_PKG_VERSION") },
            }),
            Some("tools/list") => json!({ "tools": self.tools.iter().map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.schema,
            })).collect::<Vec<_>>() }),
            Some("tools/call") => self.call(&message["params"]),
            Some("ping") => json!({}),
            other => {
                let method = other.unwrap_or_default();
                let error = json!({ "code": -32601, "message": format!("unknown method {method}") });
                return Some(json!({ "jsonrpc": "2.0", "id": id, "error": error }));
            }
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

/// An MCP server the agent reaches over streamable HTTP or by starting a
/// process that speaks stdio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConnection(pub(crate) McpServerSpec);

impl McpConnection {
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self(McpServerSpec::Http { name: name.into(), url: url.into(), headers: BTreeMap::new() })
    }

    pub fn stdio(name: impl Into<String>, command: impl Into<String>) -> Self {
        let spec =
            McpServerSpec::Stdio { name: name.into(), command: command.into(), args: Vec::new(), env: BTreeMap::new() };
        Self(spec)
    }

    /// A request header (HTTP), such as `Authorization`.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let McpServerSpec::Http { headers, .. } = &mut self.0 {
            headers.insert(key.into(), value.into());
        }
        self
    }

    /// A command-line argument (stdio).
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        if let McpServerSpec::Stdio { args, .. } = &mut self.0 {
            args.push(arg.into());
        }
        self
    }

    /// An environment variable for the process (stdio).
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let McpServerSpec::Stdio { env, .. } = &mut self.0 {
            env.insert(key.into(), value.into());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(server: &McpServer, method: &str, params: Value) -> Value {
        server.handle(&json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params })).expect("reply")
    }

    #[test]
    fn a_server_lists_and_calls_its_tools() {
        let server = McpServer::new("math")
            .tool("add", "Adds.", json!({"type": "object"}), |input| {
                Ok(json!(input["a"].as_i64().unwrap_or(0) + input["b"].as_i64().unwrap_or(0)))
            })
            .tool("greet", "Greets.", json!({"type": "object"}), |_| Ok(json!("hello")))
            .tool("fail", "Fails.", json!({"type": "object"}), |_| Err("nope".into()));
        assert_eq!(ask(&server, "initialize", json!({}))["result"]["capabilities"]["tools"], json!({}));
        let listed = ask(&server, "tools/list", json!({}));
        let names: Vec<&str> =
            listed["result"]["tools"].as_array().expect("tools").iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["add", "greet", "fail"]);
        let call =
            |name: &str, arguments: Value| ask(&server, "tools/call", json!({"name": name, "arguments": arguments}));
        assert_eq!(call("add", json!({"a": 2, "b": 3}))["result"]["content"][0]["text"], "5");
        assert_eq!(call("greet", Value::Null)["result"]["content"][0]["text"], "hello", "a string stays text");
        assert_eq!(call("fail", json!({}))["result"]["isError"], true);
        assert_eq!(call("missing", json!({}))["result"]["isError"], true);
        assert!(ask(&server, "resources/list", json!({}))["error"].is_object());
        assert!(server.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
    }

    #[test]
    fn connections_carry_their_headers_args_and_env() {
        let http = McpConnection::http("docs", "https://docs.test/mcp").header("Authorization", "Bearer t");
        assert!(matches!(&http.0, McpServerSpec::Http { headers, .. } if headers["Authorization"] == "Bearer t"));
        let stdio = McpConnection::stdio("fs", "npx").arg("-y").arg("fs-mcp").env("ROOT", "/repo");
        assert!(
            matches!(&stdio.0, McpServerSpec::Stdio { args, env, .. } if args.len() == 2 && env["ROOT"] == "/repo")
        );
    }
}
