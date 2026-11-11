use std::sync::Arc;

use serde_json::{Value, json};

use super::Lsp;
use crate::lsp::fake::ScriptedServer;
use crate::tools::{Tool, Workspace};

fn project(server: &Arc<ScriptedServer>) -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/main.rs"), "mod tools;\nfn main() {\n    tools::run(\"é\", go);\n}\n")
        .expect("main");
    std::fs::write(dir.path().join("src/tools.rs"), "/// Runs.\npub fn run(a: &str, b: u8) {}\n").expect("tools");
    let mut workspace = Workspace::new(dir.path());
    workspace.lsp = Some(server.lsp());
    (dir, workspace)
}

fn range(line: u32, character: u32) -> Value {
    json!({"start": {"line": line, "character": character}, "end": {"line": line, "character": character + 3}})
}

#[test]
fn a_definition_is_asked_at_the_position_read_shows_and_answered_as_a_place() {
    let server = ScriptedServer::answering([Ok(json!([]))]);
    let (dir, workspace) = project(&server);
    let uri = domain::lsp::uri::uri_of(&dir.path().join("src/tools.rs"));
    *server.answers.lock().expect("answers") = [Ok(json!({"uri": uri, "range": range(1, 7)}))].into();
    let input = json!({"operation": "goToDefinition", "file_path": "src/main.rs", "line": 3, "character": 18});
    let outcome = Lsp.run(&input, &workspace);
    assert!(!outcome.is_error, "{}", outcome.content);
    assert_eq!(outcome.content, "src/tools.rs:2:8: pub fn run(a: &str, b: u8) {}");
    let (method, params) = &server.asked()[0];
    assert_eq!(method, "textDocument/definition");
    assert_eq!(params["position"], json!({"line": 2, "character": 17}));
    let synced = server.synced.lock().expect("synced");
    assert_eq!((synced[0].1.as_str(), synced[0].2.contains("tools::run")), ("rust", true));
}

#[test]
fn calls_are_asked_in_two_steps() {
    let item =
        json!({"name": "run", "kind": 12, "uri": "file:///x.rs", "range": range(1, 0), "selectionRange": range(1, 7)});
    let server = ScriptedServer::answering([Ok(json!([item])), Ok(json!([]))]);
    let (_dir, workspace) = project(&server);
    let input = json!({"operation": "incomingCalls", "file_path": "src/tools.rs", "line": 2, "character": 8});
    assert_eq!(Lsp.run(&input, &workspace).content, "No callers found.");
    let methods: Vec<String> = server.asked().into_iter().map(|(method, _)| method).collect();
    assert_eq!(methods, ["textDocument/prepareCallHierarchy", "callHierarchy/incomingCalls"]);
}

#[test]
fn what_cannot_be_asked_is_an_error_that_says_why() {
    let server = ScriptedServer::answering([Err("fake: content modified".to_owned())]);
    let (_dir, workspace) = project(&server);
    let ask = |input: Value| Lsp.run(&input, &workspace);
    let busy = ask(json!({"operation": "hover", "file_path": "src/main.rs", "line": 1, "character": 1}));
    assert!(busy.is_error && busy.content.contains("still indexing"), "{}", busy.content);
    let missing = ask(json!({"operation": "documentSymbol", "file_path": "src/gone.rs"}));
    assert!(missing.is_error && missing.content.contains("src/gone.rs"), "{}", missing.content);
    let unknown = ask(json!({"operation": "documentSymbol", "file_path": "README"}));
    assert!(unknown.is_error, "{}", unknown.content);
    let bad = ask(json!({"operation": "rename", "file_path": "src/main.rs"}));
    assert!(bad.is_error && bad.content.contains("goToDefinition"), "{}", bad.content);
    let without = Lsp.run(&json!({"operation": "documentSymbol", "file_path": "src/main.rs"}), &Workspace::new("/w"));
    assert!(without.is_error, "{}", without.content);
}
