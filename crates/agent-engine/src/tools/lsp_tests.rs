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
    workspace.services.lsp = Some(server.lsp());
    workspace.policy.trust = domain::trust::Trust::Trusted;
    (dir, workspace)
}

#[test]
fn no_language_server_starts_for_a_folder_that_is_not_trusted() {
    let server = ScriptedServer::answering([Ok(json!([]))]);
    let (dir, mut workspace) = project(&server);
    workspace.policy.trust = domain::trust::Trust::Untrusted;
    for input in [
        json!({"operation": "goToDefinition", "file_path": "src/main.rs", "line": 3, "character": 18}),
        json!({"operation": "diagnostics", "file_path": "src/main.rs"}),
    ] {
        let refused = Lsp.run(&input, &workspace);
        assert!(refused.is_error && refused.content.contains("/trust"), "{}", refused.content);
    }
    let file = dir.path().join("src/main.rs");
    assert_eq!(workspace.diagnosed(&file, "fn main() {}\n"), "");
    workspace.blast_radius = true;
    assert_eq!(workspace.callers(&file, "fn main() {}\n", "fn main() { x }\n"), "");
    assert!(server.asked().is_empty() && server.synced.lock().expect("synced").is_empty(), "the server was used");
    assert_eq!(server.opened.load(std::sync::atomic::Ordering::SeqCst), 0, "a server was started");
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

fn problem(severity: u64, line: u32, message: &str) -> Value {
    json!({"severity": severity, "message": message, "range": range(line, 4)})
}

#[test]
fn a_file_s_diagnostics_are_asked_after_syncing_it_and_said_compactly() {
    let server = ScriptedServer::answering([]);
    let (_dir, workspace) = project(&server);
    *server.diagnostics.lock().expect("diagnostics") = Some(vec![problem(2, 0, "unused"), problem(1, 2, "no `go`")]);
    let asked = json!({"operation": "diagnostics", "file_path": "src/main.rs"});
    let outcome = Lsp.run(&asked, &workspace);
    assert!(!outcome.is_error, "{}", outcome.content);
    assert_eq!(
        outcome.content,
        "1 error and 1 warning in 1 file\nsrc/main.rs:3:5 error: no `go`\nsrc/main.rs:1:5 warning: unused"
    );
    assert!(server.synced.lock().expect("synced")[0].2.contains("tools::run"), "the file on disk is sent first");
    assert!(server.asked().is_empty(), "diagnostics are read from what the server published");

    *server.diagnostics.lock().expect("diagnostics") = Some(Vec::new());
    assert_eq!(Lsp.run(&asked, &workspace).content, "No errors or warnings in src/main.rs.");
    *server.diagnostics.lock().expect("diagnostics") = None;
    let silent = Lsp.run(&asked, &workspace);
    assert!(!silent.is_error && silent.content.contains("may still be checking"), "{}", silent.content);
}

#[test]
fn the_workspace_s_diagnostics_are_every_file_the_server_reported_on() {
    let server = ScriptedServer::answering([]);
    let (dir, workspace) = project(&server);
    *server.published.lock().expect("published") = vec![
        (domain::lsp::uri::uri_of(&dir.path().join("src/tools.rs")), vec![problem(2, 1, "unused `b`")]),
        (domain::lsp::uri::uri_of(&dir.path().join("src/main.rs")), vec![problem(1, 2, "no `go`")]),
        ("file:///elsewhere/lib.rs".into(), vec![problem(4, 0, "a hint")]),
    ];
    let asked = json!({"operation": "workspaceDiagnostics", "file_path": "src/main.rs"});
    assert_eq!(
        Lsp.run(&asked, &workspace).content,
        "1 error and 1 warning in 2 files\nsrc/main.rs:3:5 error: no `go`\nsrc/tools.rs:2:5 warning: unused `b`"
    );
    server.published.lock().expect("published").clear();
    assert!(Lsp.run(&asked, &workspace).content.starts_with("The language server reports no errors or warnings"));
}

#[test]
fn asking_for_diagnostics_installs_a_missing_server_only_in_a_trusted_folder() {
    let server = ScriptedServer::answering([]);
    let (_dir, mut workspace) = project(&server);
    workspace.policy.trust = domain::trust::Trust::Untrusted;
    *server.missing.lock().expect("missing") = Some("install rust-analyzer".into());
    *server.diagnostics.lock().expect("diagnostics") = Some(Vec::new());
    let asked = json!({"operation": "diagnostics", "file_path": "src/main.rs"});
    let effects = Lsp.effects(&asked, &workspace);
    assert!(!effects.iter().any(|effect| matches!(effect, crate::tools::Effect::Install { .. })), "untrusted");
    let refused = Lsp.run(&asked, &workspace);
    assert!(refused.is_error && refused.content.contains("not trusted"), "{}", refused.content);

    workspace.policy.trust = domain::trust::Trust::Trusted;
    let effects = Lsp.effects(&asked, &workspace);
    assert!(effects.iter().any(|effect| matches!(effect, crate::tools::Effect::Install { .. })), "asks first");
    let installed = Lsp.run(&asked, &workspace);
    assert!(installed.content.starts_with("Installed the "), "{}", installed.content);
    assert!(installed.content.ends_with("No errors or warnings in src/main.rs."), "{}", installed.content);
    assert_eq!(server.installs.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn what_cannot_be_asked_is_an_error_that_says_why() {
    let server =
        ScriptedServer::answering([Err(domain::lsp::answer::RequestError::Busy("fake: content modified".into()))]);
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
