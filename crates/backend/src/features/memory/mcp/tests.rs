use domain::{Author, MemoryEdit, MemoryId, Provenance};

use super::*;
use crate::features::memory::InMemoryStore;

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
        history: Vec::new(),
        uses: 0,
        last_used: 0,
    };
    assert_eq!(answer(SearchAnswer::Found(vec![memory])), "[3] procedural: Run clippy (from \"Lint\")");
}

#[test]
fn empty_answers_tell_the_agent_to_stop_searching() {
    for empty in [SearchAnswer::Nothing, SearchAnswer::AlreadyShown, SearchAnswer::Exhausted] {
        let text = answer(empty.clone());
        assert!(text.contains("Do not search memory again"), "{empty:?}: {text}");
    }
    assert!(answer(SearchAnswer::Nothing).starts_with("Nothing in this project's memory matches"));
    assert!(answer(SearchAnswer::AlreadyShown).contains("already in this conversation"));
}

fn search_call(query: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": MEMORY_SEARCH_TOOL, "arguments": { "query": query } } })
}

fn remembered(texts: &[&str]) -> Memories {
    let memories = Memories::new(Arc::new(InMemoryStore::default()));
    let by = Provenance { author: Author::User, now: 1, origin: None };
    let edits: Vec<MemoryEdit> = texts
        .iter()
        .map(|text| MemoryEdit::Remember { kind: MemoryKind::Procedural, text: (*text).into(), tags: vec![] })
        .collect();
    memories.edit(&Principal::LocalOwner, std::path::Path::new("/repo"), &edits, by).expect("edit");
    memories
}

fn text_of(server: &LocalMcp, query: &str) -> String {
    let answer = server.endpoint.handle(&search_call(query)).expect("answer");
    answer["result"]["content"][0]["text"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn a_run_never_gets_the_same_memory_twice_and_stops_searching_at_the_limit() {
    let memories = remembered(&["Run clippy before committing", "Run nextest for tests"]);
    let shown = MemorySearches::new([MemoryId(0)]);
    let server = MemoryEndpoint::for_run(memories, Principal::LocalOwner, "/repo".into(), shown);
    assert!(text_of(&server, "clippy").contains("already in this conversation"), "recalled into the prompt");
    assert_eq!(text_of(&server, "run"), "[1] procedural: Run nextest for tests");
    assert!(text_of(&server, "jira").starts_with("Nothing in this project's memory matches"));
    assert!(text_of(&server, "nextest").contains("searched memory 3 times"));
}

#[test]
fn a_run_searches_only_its_own_project_s_memory() {
    let memories = Memories::new(Arc::new(InMemoryStore::default()));
    let owner = Principal::LocalOwner;
    let by = Provenance { author: Author::User, now: 1, origin: None };
    let edit = MemoryEdit::Remember {
        kind: MemoryKind::Procedural,
        text: "Run clippy before committing".into(),
        tags: vec![],
    };
    memories.edit(&owner, std::path::Path::new("/repo"), &[edit], by).expect("edit");
    let call = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": MEMORY_SEARCH_TOOL, "arguments": { "query": "clippy" } } });
    let text = |project: &str| {
        let server =
            MemoryEndpoint::for_run(memories.clone(), owner.clone(), project.into(), MemorySearches::default());
        assert_eq!(server.name, MEMORY_SERVER);
        server.endpoint.handle(&call).expect("answer")["result"]["content"][0]["text"].as_str().map(str::to_owned)
    };
    assert_eq!(text("/repo").as_deref(), Some("[0] procedural: Run clippy before committing"));
    assert!(text("/other").is_some_and(|text| text.starts_with("Nothing in this project's memory matches")));
}
