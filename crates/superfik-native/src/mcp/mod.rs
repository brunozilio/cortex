//! MCP tools for the native engine: Superfik's own memory server, the
//! SDK host's servers over streamable HTTP or as processes, the host's
//! in-process servers ([`LocalTransport`]), the project's and the user's
//! configured servers and installed plugins'; each tool is offered as
//! `mcp__<server>__<tool>`.

mod configured;
mod http;
mod local;
mod servers;
mod stdio;
mod tool;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use superfik_core::PromptRequest;

pub use self::configured::{McpSources, list};
pub use self::http::{HttpTransport, parse};
pub use self::local::LocalTransport;
pub use self::servers::{ServerSpec, parse_config, servers};
pub use self::stdio::StdioTransport;
use self::tool::outcome;
pub use self::tool::{McpTool, builtin_access};
use crate::model::ToolSpec;
use crate::model::http::Http;
use crate::tools::{Outcome, Tool};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub trait Transport: Send + Sync {
    fn request(&self, message: &Value) -> Result<Value, String>;
    fn request_stopped(&self, message: &Value, stop: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        self.request(message)
    }
    fn notify(&self, message: &Value) -> Result<(), String>;
    fn notify_stopped(&self, message: &Value, stop: &std::sync::atomic::AtomicBool) -> Result<(), String> {
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        self.notify(message)
    }
}

pub struct McpServer {
    name: String,
    transport: Box<dyn Transport>,
    next: AtomicU64,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer").field("name", &self.name).finish_non_exhaustive()
    }
}

impl McpServer {
    pub fn new(name: impl Into<String>, transport: Box<dyn Transport>) -> Self {
        Self { name: name.into(), transport, next: AtomicU64::new(1) }
    }

    pub fn open(spec: &ServerSpec, http: &Arc<dyn Http>) -> Result<Self, String> {
        Self::open_stopped(spec, http, &std::sync::atomic::AtomicBool::new(false))
    }

    pub fn open_stopped(
        spec: &ServerSpec,
        http: &Arc<dyn Http>,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, String> {
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let transport: Box<dyn Transport> = match spec {
            ServerSpec::Http { name, url, headers } => {
                Box::new(HttpTransport::new(name, url, headers, Arc::clone(http)))
            }
            ServerSpec::Stdio { name, command, args, env } => {
                Box::new(StdioTransport::spawn(name, command, args, env)?)
            }
        };
        Ok(Self::new(spec.name(), transport))
    }

    fn rpc_stopped(&self, method: &str, params: Value, stop: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let reply = self
            .transport
            .request_stopped(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }), stop)?;
        if let Some(error) = reply.get("error") {
            let message = error["message"].as_str().unwrap_or("the call failed");
            return Err(format!("{}: {message}", self.name));
        }
        Ok(reply["result"].clone())
    }

    pub fn tools(self: &Arc<Self>) -> Result<Vec<McpTool>, String> {
        self.tools_stopped(&std::sync::atomic::AtomicBool::new(false))
    }
    pub fn tools_stopped(self: &Arc<Self>, stop: &std::sync::atomic::AtomicBool) -> Result<Vec<McpTool>, String> {
        let client = json!({ "name": "superfik", "version": env!("CARGO_PKG_VERSION") });
        self.rpc_stopped(
            "initialize",
            json!({ "protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": client }),
            stop,
        )?;
        self.transport.notify_stopped(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), stop)?;
        let mut tools = Vec::new();
        let mut params = json!({});
        let mut cursors = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for _ in 0..100 {
            let listed = self.rpc_stopped("tools/list", params, stop)?;
            let page =
                listed["tools"].as_array().ok_or_else(|| format!("{} sent an invalid tools/list result", self.name))?;
            for tool in page {
                let name = tool["name"].as_str().filter(|s| !s.is_empty()).ok_or("MCP tool has no name")?.to_owned();
                if !names.insert(name.clone()) {
                    return Err("MCP catalog contains a duplicate tool name".into());
                }
                if tools.len() >= 10_000 {
                    return Err("MCP catalog exceeds the tool limit".into());
                }
                let spec = ToolSpec {
                    name: format!("mcp__{}__{name}", self.name),
                    description: tool["description"].as_str().unwrap_or_default().to_owned(),
                    schema: tool.get("inputSchema").cloned().unwrap_or_else(|| json!({"type":"object"})),
                };
                tools.push(McpTool { server: Arc::clone(self), name, spec });
            }
            match listed.get("nextCursor") {
                None | Some(Value::Null) => return Ok(tools),
                Some(Value::String(cursor)) => {
                    if !cursors.insert(cursor.clone()) {
                        return Err("MCP catalog repeated its pagination cursor".into());
                    }
                    params = json!({"cursor":cursor});
                }
                Some(_) => return Err("MCP catalog sent a non-string pagination cursor".into()),
            }
        }
        Err("MCP catalog exceeded the pagination limit".into())
    }

    fn call(&self, name: &str, arguments: &Value, stop: &std::sync::atomic::AtomicBool) -> Outcome {
        let arguments = if arguments.is_null() { json!({}) } else { arguments.clone() };
        match self.transport.request_stopped(&json!({"jsonrpc":"2.0","id":self.next.fetch_add(1,Ordering::Relaxed),"method":"tools/call","params":{"name":name,"arguments":arguments}}),stop).and_then(|reply| {
            if let Some(error)=reply.get("error") {Err(error["message"].as_str().unwrap_or("MCP call failed").to_owned())} else {Ok(reply["result"].clone())}
        }) {
            Ok(result) => outcome(&result),
            Err(error) => Outcome::error(error),
        }
    }
}

pub fn tools_stopped(
    request: &PromptRequest,
    sources: &McpSources,
    http: &Arc<dyn Http>,
    stop: &std::sync::atomic::AtomicBool,
) -> Vec<Box<dyn Tool>> {
    let listed = |name: &str, server: Result<McpServer, String>| {
        server.and_then(|server| Arc::new(server).tools_stopped(stop)).unwrap_or_else(|error| {
            tracing::warn!(%error, server = name, "an MCP server's tools are not available to this run");
            Vec::new()
        })
    };
    let local = request.credentials.mcp.iter().take_while(|_| !stop.load(Ordering::Relaxed)).flat_map(|local| {
        let transport = Box::new(LocalTransport::new(Arc::clone(&local.endpoint)));
        listed(&local.name, Ok(McpServer::new(local.name.clone(), transport)))
    });
    let remote = servers(request, sources)
        .into_iter()
        .take_while(|_| !stop.load(Ordering::Relaxed))
        .flat_map(|spec| listed(spec.name(), McpServer::open_stopped(&spec, http, stop)));
    local.chain(remote).map(|tool| Box::new(tool) as Box<dyn Tool>).collect()
}

#[cfg(test)]
mod tests;
