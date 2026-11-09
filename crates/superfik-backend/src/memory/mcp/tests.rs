use std::io::{Read, Write};
use std::net::TcpStream;

use superfik_core::{Author, MemoryEdit, MemoryId, Provenance};

use super::*;
use crate::memory::InMemoryStore;

fn ask(method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params });
    respond(&message, |query, limit| format!("{query}:{limit}")).expect("answer")
}

#[test]
fn speaks_enough_mcp_for_one_tool() {
    let init = ask("initialize", json!({ "protocolVersion": "2025-03-26" }));
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(init["result"]["serverInfo"]["name"], MEMORY_SERVER);
    assert_eq!(ask("tools/list", Value::Null)["result"]["tools"][0]["name"], MEMORY_SEARCH_TOOL);
    let call =
        ask("tools/call", json!({ "name": MEMORY_SEARCH_TOOL, "arguments": { "query": " lint ", "limit": 99 } }));
    assert_eq!(call["result"]["content"][0]["text"], "lint:25");
    let empty = ask("tools/call", json!({ "name": MEMORY_SEARCH_TOOL, "arguments": {} }));
    assert_eq!(empty["result"]["isError"], true);
    assert_eq!(ask("tools/call", json!({ "name": "other" }))["error"]["code"], -32602);
    assert_eq!(ask("resources/list", Value::Null)["error"]["code"], -32601);
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert!(respond(&note, |_, _| String::new()).is_none());
}

#[test]
fn listings_name_ids_kinds_and_sessions() {
    let memory = Memory {
        id: MemoryId(3),
        kind: MemoryKind::Procedural,
        text: "Run clippy".into(),
        tags: vec![],
        created: 0,
        origin: Some("Lint".into()),
        author: Author::Agent,
    };
    assert_eq!(listing(&[memory]), "[3] procedural: Run clippy (from \"Lint\")");
    assert_eq!(listing(&[]), "No memory matches.");
}

fn post(url: &str, token: &str, body: &Value) -> (u16, String) {
    let address = url.trim_start_matches("http://").trim_end_matches("/mcp");
    let mut stream = TcpStream::connect(address).expect("connect");
    let body = body.to_string();
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read");
    let status = response.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default().to_owned();
    (status, body)
}

#[test]
fn a_run_searches_its_own_memory_until_revoked() {
    let memories = Memories::plain(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    let by = Provenance { author: Author::User, now: 1, origin: None };
    let edit = MemoryEdit::Remember {
        kind: MemoryKind::Procedural,
        text: "Run clippy before committing".into(),
        tags: vec![],
    };
    memories.edit(&owner, std::path::Path::new("/repo"), &[edit], by);
    let server = McpServer::start(memories).expect("server");
    let tool = server.grant(&owner, "/repo".into());
    let call = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": MEMORY_SEARCH_TOOL, "arguments": { "query": "clippy" } } });
    let (status, body) = post(&tool.url, &tool.token, &call);
    assert_eq!(status, 200);
    assert!(body.contains("[0] procedural: Run clippy before committing"), "{body}");
    assert_eq!(post(&tool.url, "wrong", &call).0, 401);
    server.revoke(&tool.token);
    assert_eq!(post(&tool.url, &tool.token, &call).0, 401);
}
