use std::path::Path;
use std::sync::atomic::AtomicBool;

use serde_json::json;

use crate::fake::ScriptedServer;

fn range(line: u32) -> serde_json::Value {
    json!({ "start": { "line": line, "character": 3 }, "end": { "line": line + 2, "character": 1 } })
}

#[test]
fn the_changed_function_s_callers_in_other_files_are_reported() {
    let symbols = json!([{ "name": "retry", "kind": 12, "range": range(4), "selectionRange": range(4) }]);
    let references = json!([
        { "uri": "file:///w/src/a.rs", "range": range(9) },
        { "uri": "file:///w/src/client.rs", "range": range(30) }
    ]);
    let server = ScriptedServer::answering([Ok(symbols), Ok(references)]);

    let block = server
        .lsp()
        .callers(Path::new("/w/src/client.rs"), "fn retry() {}", (5, 5), Path::new("/w"), &AtomicBool::new(false))
        .expect("callers");

    assert!(block.contains("- retry (1 place): src/a.rs:10"), "{block}");
    let asked: Vec<String> = server.asked().into_iter().map(|(method, _)| method).collect();
    assert_eq!(asked, ["textDocument/documentSymbol", "textDocument/references"]);
}

#[test]
fn an_edit_outside_any_symbol_or_a_file_no_server_handles_says_nothing() {
    let symbols = json!([{ "name": "retry", "kind": 12, "range": range(4), "selectionRange": range(4) }]);
    let server = ScriptedServer::answering([Ok(symbols)]);
    let stop = AtomicBool::new(false);

    assert_eq!(server.lsp().callers(Path::new("/w/src/client.rs"), "", (40, 41), Path::new("/w"), &stop), None);
    assert_eq!(server.lsp().callers(Path::new("/w/notes.txt"), "", (0, 0), Path::new("/w"), &stop), None);
}
