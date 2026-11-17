pub mod channel;
mod configured;
pub mod extras;
mod http;
mod http_listen;
mod local;
mod pool;
mod script_server;
mod servers;
mod sse;
mod sse_listen;
mod stdio;
mod stdio_sandbox;
mod tool;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use domain::PromptRequest;
use serde_json::{Value, json};

pub use self::configured::{McpSources, list, listing, project_config, sources};
pub use self::http::{HttpTransport, parse};
pub use self::local::LocalTransport;
pub use self::pool::McpPool;
pub use self::servers::{ServerSpec, StdioSandbox, parse_config, servers};
pub use self::stdio::StdioTransport;
use self::tool::outcome;
pub use self::tool::{McpTool, remote_effects};
use crate::model::ToolSpec;
use crate::model::http::Http;
use crate::tools::{Outcome, Tool};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(600);
pub(crate) const MESSAGE_AT_MOST: usize = 16 * 1024 * 1024;

pub type Listener = Arc<dyn Fn(&str, &Value) + Send + Sync>;

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
    fn listen(&self) {}
    fn alive(&self) -> bool {
        true
    }
    fn close(&self) {}
}

pub struct McpServer {
    name: String,
    transport: Box<dyn Transport>,
    next: AtomicU64,
    capabilities: std::sync::Mutex<Value>,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer").field("name", &self.name).finish_non_exhaustive()
    }
}

impl McpServer {
    pub fn new(name: impl Into<String>, transport: Box<dyn Transport>) -> Self {
        Self { name: name.into(), transport, next: AtomicU64::new(1), capabilities: std::sync::Mutex::default() }
    }

    pub fn open(spec: &ServerSpec, http: &Arc<dyn Http>) -> Result<Self, String> {
        Self::open_stopped(spec, http, &std::sync::atomic::AtomicBool::new(false))
    }

    pub fn open_stopped(
        spec: &ServerSpec,
        http: &Arc<dyn Http>,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, String> {
        Self::open_listening(spec, http, stop, None)
    }

    pub fn open_listening(
        spec: &ServerSpec,
        http: &Arc<dyn Http>,
        stop: &std::sync::atomic::AtomicBool,
        listener: Option<Listener>,
    ) -> Result<Self, String> {
        if stop.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let transport: Box<dyn Transport> = match spec {
            ServerSpec::Http { name, url, headers } => {
                Box::new(HttpTransport::listening(name, url, headers, Arc::clone(http), listener))
            }
            ServerSpec::Sse { name, url, headers } => {
                Box::new(sse::SseTransport::connect(name, url, headers, Arc::clone(http), stop, listener)?)
            }
            ServerSpec::Script { path, cwd, access, origin, confinement, judge, .. } => {
                let server = script_server::ScriptServer::open(spec.name(), path, cwd, *access)?
                    .coming_from(*origin)
                    .confined_to(confinement.clone())
                    .judged_by(Arc::clone(&judge.0));
                Box::new(LocalTransport::new(Arc::new(server)))
            }
            ServerSpec::Stdio { name, command, args, env, sandbox } => {
                Box::new(StdioTransport::spawn_in(name, command, args, env, sandbox, listener)?)
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
        self.initialize_stopped(stop)?;
        self.listed_tools_stopped(stop)
    }

    pub fn initialize_stopped(&self, stop: &std::sync::atomic::AtomicBool) -> Result<(), String> {
        let client = json!({ "name": brand::COMMAND, "version": env!("CARGO_PKG_VERSION") });
        let answer = self.rpc_stopped(
            "initialize",
            json!({ "protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": client }),
            stop,
        )?;
        if let Ok(mut capabilities) = self.capabilities.lock() {
            *capabilities = answer["capabilities"].clone();
        }
        self.transport.notify_stopped(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), stop)?;
        self.transport.listen();
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn alive(&self) -> bool {
        self.transport.alive()
    }

    pub fn close(&self) {
        self.transport.close();
    }

    pub fn capabilities(&self) -> Value {
        self.capabilities.lock().map(|capabilities| capabilities.clone()).unwrap_or_default()
    }

    pub fn offers(&self, capability: &str) -> bool {
        self.capabilities.lock().is_ok_and(|capabilities| capabilities.get(capability).is_some_and(Value::is_object))
    }

    pub fn ask(&self, method: &str, params: Value, stop: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        self.rpc_stopped(method, params, stop)
    }

    pub fn listed_tools_stopped(
        self: &Arc<Self>,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<McpTool>, String> {
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
    tools_of(&connect_stopped(request, sources, http, stop), stop)
}

pub fn connect_stopped(
    request: &PromptRequest,
    sources: &McpSources,
    http: &Arc<dyn Http>,
    stop: &std::sync::atomic::AtomicBool,
) -> Vec<Arc<McpServer>> {
    connect_judged(request, sources, http, stop, &Arc::default())
}

pub fn connect_judged(
    request: &PromptRequest,
    sources: &McpSources,
    http: &Arc<dyn Http>,
    stop: &std::sync::atomic::AtomicBool,
    judge: &Arc<crate::tools::host_judge::HostJudge>,
) -> Vec<Arc<McpServer>> {
    let mut connected = in_process(request, stop);
    let remote = wanted(request, sources, judge)
        .into_iter()
        .take_while(|_| !stop.load(Ordering::Relaxed))
        .filter_map(|spec| ready(spec.name(), McpServer::open_stopped(&spec, http, stop), stop));
    connected.extend(remote);
    connected
}

fn ready(
    name: &str,
    server: Result<McpServer, String>,
    stop: &std::sync::atomic::AtomicBool,
) -> Option<Arc<McpServer>> {
    server
        .and_then(|server| server.initialize_stopped(stop).map(|()| Arc::new(server)))
        .inspect_err(|error| tracing::warn!(%error, server = name, "an MCP server is not available to this run"))
        .ok()
}

fn in_process(request: &PromptRequest, stop: &std::sync::atomic::AtomicBool) -> Vec<Arc<McpServer>> {
    request
        .credentials
        .mcp
        .iter()
        .take_while(|_| !stop.load(Ordering::Relaxed))
        .filter_map(|local| {
            let transport = Box::new(LocalTransport::new(Arc::clone(&local.endpoint)));
            ready(&local.name, Ok(McpServer::new(local.name.clone(), transport)), stop)
        })
        .collect()
}

fn wanted(
    request: &PromptRequest,
    sources: &McpSources,
    judge: &Arc<crate::tools::host_judge::HostJudge>,
) -> Vec<ServerSpec> {
    servers(request, sources)
        .into_iter()
        .filter(|spec| !request.credentials.mcp.iter().any(|local| local.name == spec.name()))
        .map(|spec| spec.signed_in(request.credentials.mcp_tokens.as_ref()).judged_by(judge))
        .collect()
}

pub fn tools_of(servers: &[Arc<McpServer>], stop: &std::sync::atomic::AtomicBool) -> Vec<Box<dyn Tool>> {
    let mut tools: Vec<Box<dyn Tool>> = servers
        .iter()
        .flat_map(|server| {
            server.listed_tools_stopped(stop).unwrap_or_else(|error| {
                tracing::warn!(%error, server = server.name(), "an MCP server's tools are not available to this run");
                Vec::new()
            })
        })
        .map(|tool| Box::new(tool) as Box<dyn Tool>)
        .collect();
    tools.extend(extras::resource_tools(servers));
    tools
}

#[cfg(test)]
mod tests;
