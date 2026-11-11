//! The memory search tool, served to agents over MCP in process: each run
//! gets its own [`MemoryEndpoint`] on its credentials, bound to its owner
//! and project, so an agent can only search the memory it was briefed with.
//!
//! [`respond`] is the protocol and is pure.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use domain::mcp::{LocalMcp, McpEndpoint, RpcError, text_result};
use domain::memory::{MEMORY_SEARCH_TOOL, MEMORY_SERVER};
use domain::{Memory, MemoryKind, Principal};

use super::Memories;

const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 25;

pub fn respond(message: &Value, search: impl Fn(&str, usize) -> String) -> Option<Value> {
    domain::mcp::respond(
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

fn call(name: &str, arguments: &Value, search: impl Fn(&str, usize) -> String) -> Result<Value, RpcError> {
    if name != MEMORY_SEARCH_TOOL {
        return Err((-32602, format!("unknown tool: {name}")));
    }
    let query = arguments.get("query").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    if query.is_empty() {
        return Ok(text_result("Give a query to search for.", true));
    }
    let limit =
        arguments.get("limit").and_then(Value::as_u64).map_or(DEFAULT_LIMIT, |n| n as usize).clamp(1, MAX_LIMIT);
    Ok(text_result(search(query, limit), false))
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

/// One run's memory search: its owner's memory of its project.
pub struct MemoryEndpoint {
    memories: Memories,
    owner: Principal,
    project: PathBuf,
}

impl MemoryEndpoint {
    /// The server a run gets, named [`MEMORY_SERVER`].
    pub fn for_run(memories: Memories, owner: Principal, project: PathBuf) -> LocalMcp {
        LocalMcp::new(MEMORY_SERVER, Arc::new(Self { memories, owner, project }))
    }
}

impl McpEndpoint for MemoryEndpoint {
    fn handle(&self, message: &Value) -> Option<Value> {
        respond(message, |query, limit| listing(&self.memories.find(&self.owner, &self.project, query, limit)))
    }
}

#[cfg(test)]
mod tests;
