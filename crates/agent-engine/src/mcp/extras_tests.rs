use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::{ModelOption, PromptRequest};
use serde_json::{Value, json};

use super::{expand_prompt, resource_tools};
use crate::mcp::connect_stopped;
use crate::model::http::{Http, ScriptedHttp};
use crate::tools::Workspace;

/// A server with a resource and a prompt.
struct Docs;

impl domain::mcp::McpEndpoint for Docs {
    fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id")?.clone();
        let params = &message["params"];
        let result = match message["method"].as_str() {
            Some("initialize") => json!({ "capabilities": { "resources": {}, "prompts": {}, "tools": {} } }),
            Some("tools/list") => json!({ "tools": [] }),
            Some("resources/list") => {
                json!({ "resources": [{ "uri": "docs://intro", "name": "Intro", "description": "Start here" }] })
            }
            Some("resources/read") if params["uri"] == "docs://intro" => {
                json!({ "contents": [{ "uri": "docs://intro", "text": "Welcome to the docs." }] })
            }
            Some("prompts/list") => json!({ "prompts": [{ "name": "review", "arguments": [
                { "name": "file" }, { "name": "focus" }
            ] }] }),
            Some("prompts/get") => json!({ "messages": [{ "role": "user", "content": { "type": "text", "text":
                format!("Review {} for {}.", params["arguments"]["file"].as_str().unwrap_or("?"),
                    params["arguments"]["focus"].as_str().unwrap_or("?")) } }] }),
            _ => json!({}),
        };
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
}

fn servers() -> Vec<Arc<crate::mcp::McpServer>> {
    let mut request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
    request.credentials.mcp = vec![domain::mcp::LocalMcp::new("docs", Arc::new(Docs))];
    let http: Arc<dyn Http> = Arc::new(ScriptedHttp::new([]));
    let sources = crate::mcp::McpSources::default();
    connect_stopped(&request, &sources, &http, &AtomicBool::new(false))
}

#[test]
fn a_server_s_resources_are_listed_and_read() {
    let tools = resource_tools(&servers());
    let names: Vec<String> = tools.iter().map(|tool| tool.spec().name).collect();
    assert_eq!(names, ["ListMcpResources", "ReadMcpResource"]);
    let workspace = Workspace::new("/repo");
    let listed = tools[0].run(&json!({ "server": "docs" }), &workspace);
    assert_eq!(listed.content, "docs: Intro <docs://intro> Start here");
    let read = tools[1].run(&json!({ "server": "docs", "uri": "docs://intro" }), &workspace);
    assert_eq!(read.content, "Welcome to the docs.");
}

#[test]
fn a_server_s_rules_cover_its_resources() {
    use domain::permission_rules::{PermissionRules, RuleKind};
    let mut workspace = Workspace::new("/repo");
    workspace.permissions = PermissionRules::default().with(RuleKind::Deny, "mcp__docs");
    let read = json!({ "server": "docs", "uri": "docs://intro" });
    assert_eq!(workspace.ruling("ReadMcpResource", "docs://intro", &read).map(|(kind, _)| kind), Some(RuleKind::Deny));
    let list = json!({ "server": "docs" });
    assert_eq!(workspace.ruling("ListMcpResources", "docs", &list).map(|(kind, _)| kind), Some(RuleKind::Deny));
    let other = json!({ "server": "wiki", "uri": "x" });
    assert_eq!(workspace.ruling("ReadMcpResource", "x", &other), None);
}

#[test]
fn a_typed_mcp_prompt_is_filled_with_the_words_after_it() {
    let servers = servers();
    let expanded = expand_prompt(&servers, "/mcp__docs__review src/main.rs error handling", &AtomicBool::new(false));
    assert_eq!(expanded, Some(Ok("Review src/main.rs for error handling.".into())));
    assert_eq!(expand_prompt(&servers, "/review it", &AtomicBool::new(false)), None, "not an MCP prompt");
    assert!(matches!(expand_prompt(&servers, "/mcp__docs__missing", &AtomicBool::new(false)), Some(Err(_))));
}
