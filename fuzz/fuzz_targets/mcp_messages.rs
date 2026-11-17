#![no_main]

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use agent_engine::mcp::{McpError, McpServer, Transport, outcome, parse};
use agent_engine::tools::Tool;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

#[derive(Debug)]
struct Scripted(Mutex<VecDeque<Value>>);

impl Transport for Scripted {
    fn request(&self, _message: &Value) -> Result<Value, McpError> {
        let mut replies = self.0.lock().map_err(|_| McpError::Cancelled)?;
        replies.pop_front().ok_or(McpError::Cancelled)
    }

    fn notify(&self, _message: &Value) -> Result<(), McpError> {
        Ok(())
    }
}

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let replies: Vec<Value> = text.split('\u{1e}').filter_map(parse).collect();
    for reply in &replies {
        let _ = outcome(reply);
        let _ = outcome(&reply["result"]);
    }

    let server = Arc::new(McpServer::new("fuzz", Box::new(Scripted(Mutex::new(replies.into())))));
    if let Ok(tools) = server.tools() {
        assert!(tools.len() <= 10_000);
        let mut names = HashSet::new();
        for tool in &tools {
            let spec = tool.spec();
            assert!(spec.name.starts_with("mcp__fuzz__") && spec.name.len() > "mcp__fuzz__".len(), "{}", spec.name);
            assert!(names.insert(spec.name), "a tool name came twice");
        }
    }
    let _ = server.capabilities();
    let _ = server.offers("resources");
});
