use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use agent_engine::lsp::{Downloads, LanguageServer, LanguageServers, Lsp, LspPool, Processes, Starter};
use domain::lsp::ServerSpec;
use agent_engine::tools::{self, Tool, Workspace};
use serde_json::json;

#[test]
#[ignore = "needs clangd on PATH"]
fn clangd_answers_symbols_and_reports_an_edit_s_error() {
    let pool = Arc::new(LspPool::default());
    let specs = Arc::new(domain::lsp::defaults());
    assert!(pool.available(&specs).iter().any(|spec| spec.name == "clangd"), "clangd is not installed");
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
    let edited = tools::named("Edit").expect("Edit").run(&edit, &workspace);
    assert!(
        edited.content.contains("<diagnostics>") && edited.content.contains("undefined_name"),
        "{}",
        edited.content
    );
}

struct NothingOnPath;

impl Starter for NothingOnPath {
    fn find(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn start(
        &self,
        spec: &ServerSpec,
        program: &Path,
        root: &Path,
        stop: &AtomicBool,
    ) -> Result<Arc<dyn LanguageServer>, String> {
        Processes.start(spec, program, root, stop)
    }
}

#[test]
#[ignore = "downloads every built-in server from the network"]
fn a_missing_server_is_installed_and_then_answers() {
    let data = tempfile::tempdir().expect("data");
    let pool = Arc::new(
        LspPool::installing(NothingOnPath, Duration::from_secs(600), data.path().into(), Downloads),
    );
    let cases = [
        ("rust-analyzer", "Cargo.toml", "[package]\nname = \"w\"\nversion = \"0.1.0\"\n", "lib.rs", "pub fn add() {}\n"),
        ("typescript", "package.json", "{}\n", "app.ts", "export function add() {}\n"),
        ("clangd", "CMakeLists.txt", "\n", "main.c", "int add(void) { return 0; }\n"),
        ("pyright", "pyproject.toml", "\n", "app.py", "def add():\n    return 0\n"),
        ("gopls", "go.mod", "module w\n\ngo 1.21\n", "main.go", "package main\n\nfunc add() int { return 0 }\n"),
    ];
    for (server, marker, manifest, file, source) in cases {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join(marker), manifest).expect("marker");
        std::fs::write(dir.path().join(file), source).expect("file");
        let mut workspace = Workspace::new(dir.path());
        let specs = Arc::new(domain::lsp::defaults());
        workspace.lsp = Some(Lsp { servers: pool.clone(), specs });
        let outline = tools::Lsp.run(&json!({"operation": "documentSymbol", "file_path": file}), &workspace);
        assert!(!outline.is_error && outline.content.contains("add"), "{server}: {}", outline.content);
        assert!(outline.content.starts_with(&format!("Installed the {server} language server")), "{}", outline.content);
        assert!(data.path().join(server).is_dir(), "{server} was not installed");
    }
}
