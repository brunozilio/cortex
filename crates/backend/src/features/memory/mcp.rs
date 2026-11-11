use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use domain::mcp::{LocalMcp, McpEndpoint, RpcError, text_result};
use domain::memory::{MEMORY_SEARCH_TOOL, MEMORY_SERVER, MemorySearches, SearchAnswer};
use domain::{Memory, MemoryKind, Principal};
use serde_json::{Value, json};

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
                        Matches by meaning and by words; ask in any language. Memories already in the conversation \
                        are not returned again. Search once per topic: an empty answer is final.",
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

pub fn answer(answer: SearchAnswer) -> String {
    const STOP: &str = "Do not search memory again for this; carry on with the task.";
    match answer {
        SearchAnswer::Found(memories) => listing(&memories),
        SearchAnswer::Nothing => format!("Nothing in this project's memory matches. {STOP}"),
        SearchAnswer::AlreadyShown => {
            format!("Every memory that matches is already in this conversation. {STOP}")
        }
        SearchAnswer::Exhausted => format!(
            "You have searched memory {} times this turn, the most a turn allows. {STOP}",
            MemorySearches::LIMIT
        ),
    }
}

fn listing(memories: &[Memory]) -> String {
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

pub struct MemoryEndpoint {
    memories: Memories,
    owner: Principal,
    project: PathBuf,
    searches: Mutex<MemorySearches>,
}

impl MemoryEndpoint {
    pub fn for_run(memories: Memories, owner: Principal, project: PathBuf, searches: MemorySearches) -> LocalMcp {
        LocalMcp::new(MEMORY_SERVER, Arc::new(Self { memories, owner, project, searches: Mutex::new(searches) }))
    }
}

impl McpEndpoint for MemoryEndpoint {
    fn handle(&self, message: &Value) -> Option<Value> {
        respond(message, |query, limit| {
            let mut searches = self.searches.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            answer(self.memories.find(&self.owner, &self.project, query, limit, &mut searches))
        })
    }
}

#[cfg(test)]
mod tests;
