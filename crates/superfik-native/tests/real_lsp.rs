//! Against a real language server (clangd), when installed:
//! `cargo test -p superfik-native --test real_lsp -- --ignored`.

use std::sync::Arc;

use serde_json::json;
use superfik_native::lsp::{LanguageServers, Lsp, LspPool};
use superfik_native::tools::{self, Tool, Workspace};

#[test]
#[ignore = "needs clangd on PATH"]
fn clangd_answers_symbols_and_reports_an_edit_s_error() {
    let pool = Arc::new(LspPool::default());
    let specs = Arc::new(superfik_core::lsp::defaults());
    assert!(pool.any_installed(&specs), "clangd is not installed");
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("main.c");
    std::fs::write(&file, "int add(int a, int b) { return a + b; }\nint main(void) { return add(1, 2); }\n")
        .expect("file");
    let mut workspace = Workspace::new(dir.path());
    workspace.lsp = Some(Lsp { servers: pool, specs });

    let outline = tools::Lsp.run(&json!({"operation": "documentSymbol", "file_path": "main.c"}), &workspace);
    assert!(outline.content.contains("function add"), "{}", outline.content);
    let definition = json!({"operation": "goToDefinition", "file_path": "main.c", "line": 2, "character": 25});
    let found = tools::Lsp.run(&definition, &workspace);
    assert!(found.content.starts_with("main.c:1:"), "{}", found.content);

    workspace.mark_read(&file);
    let edit = json!({"file_path": "main.c", "old_string": "add(1, 2)", "new_string": "add(1, undefined_name)"});
    let edited = tools::Edit.run(&edit, &workspace);
    assert!(
        edited.content.contains("<diagnostics>") && edited.content.contains("undefined_name"),
        "{}",
        edited.content
    );
}
