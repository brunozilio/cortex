//! The memory search tool, served to agents over MCP (streamable HTTP,
//! JSON responses only) on a loopback port. Each run gets a bearer token
//! naming its owner and project; the token is revoked when the run ends,
//! so an agent can only search the memory it was briefed with.
//!
//! [`respond`] is the protocol and is pure; [`McpServer`] is the socket.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use superfik_core::memory::{MEMORY_SEARCH_TOOL, MEMORY_SERVER};
use superfik_core::{Memory, MemoryKind, MemoryTool, Principal};

use super::Memories;
use crate::loopback;

const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 25;

pub fn respond(message: &Value, search: impl Fn(&str, usize) -> String) -> Option<Value> {
    loopback::respond(
        message,
        MEMORY_SERVER,
        || vec![tool_definition()],
        |name, arguments| call(name, arguments, search),
    )
}

fn tool_definition() -> Value {
    json!({
        "name": MEMORY_SEARCH_TOOL,
        "description": "Search this project's memory from earlier sessions: facts about the project and the user \
                        (semantic), how things are done here (procedural) and what happened before (episodic). \
                        Matches by meaning and by words; ask in any language.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "What to look for." },
                "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "Most results to return (default 8)." }
            },
            "required": ["query"]
        }
    })
}

fn call(name: &str, arguments: &Value, search: impl Fn(&str, usize) -> String) -> Result<Value, loopback::RpcError> {
    if name != MEMORY_SEARCH_TOOL {
        return Err(loopback::unknown_tool(name));
    }
    let query = arguments.get("query").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    if query.is_empty() {
        return Ok(loopback::text("Give a query to search for.", true));
    }
    let limit =
        arguments.get("limit").and_then(Value::as_u64).map_or(DEFAULT_LIMIT, |n| n as usize).clamp(1, MAX_LIMIT);
    Ok(loopback::text(search(query, limit), false))
}

pub fn listing(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return "No memory matches.".to_owned();
    }
    memories
        .iter()
        .map(|m| {
            let kind = match m.kind {
                MemoryKind::Episodic => "episodic",
                MemoryKind::Semantic => "semantic",
                MemoryKind::Procedural => "procedural",
            };
            let origin = m.origin.as_deref().map(|o| format!(" (from \"{o}\")")).unwrap_or_default();
            format!("[{}] {kind}: {}{origin}", m.id.0, m.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

type Grants = Arc<Mutex<HashMap<String, (Principal, PathBuf)>>>;

#[derive(Clone)]
pub struct McpServer {
    url: String,
    grants: Grants,
}

impl std::fmt::Debug for McpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer").field("url", &self.url).finish_non_exhaustive()
    }
}

impl McpServer {
    pub fn start(memories: Memories) -> Option<Self> {
        let server = match tiny_http::Server::http("127.0.0.1:0") {
            Ok(server) => server,
            Err(error) => {
                tracing::warn!(%error, "memory tool unavailable");
                return None;
            }
        };
        let port = server.server_addr().to_ip()?.port();
        let grants: Grants = Arc::default();
        let served = Arc::clone(&grants);
        std::thread::Builder::new()
            .name("superfik-memory-mcp".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    serve(request, &served, &memories);
                }
            })
            .ok()?;
        Some(Self { url: format!("http://127.0.0.1:{port}/mcp"), grants })
    }

    pub fn grant(&self, owner: &Principal, project: PathBuf) -> MemoryTool {
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.lock().insert(token.clone(), (owner.clone(), project));
        MemoryTool { url: self.url.clone(), token }
    }

    pub fn revoke(&self, token: &str) {
        self.lock().remove(token);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, (Principal, PathBuf)>> {
        self.grants.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn serve(mut request: tiny_http::Request, grants: &Grants, memories: &Memories) {
    let grant = loopback::bearer(&request)
        .and_then(|token| grants.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&token).cloned());
    let Some((owner, project)) = grant else { return loopback::reply(request, 401, None) };
    let message = match loopback::read_message(&mut request) {
        Ok(message) => message,
        Err(status) => return loopback::reply(request, status, None),
    };
    let search = |query: &str, limit: usize| listing(&memories.find(&owner, &project, query, limit));
    match respond(&message, search) {
        Some(answer) => loopback::reply(request, 200, Some(answer)),
        None => loopback::reply(request, 202, None),
    }
}

#[cfg(test)]
mod tests;
