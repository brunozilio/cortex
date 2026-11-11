//! Language servers reach a run: the `LSP` tool when one is installed.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use domain::lsp::ServerSpec;
use domain::{EngineEvent, PromptRequest};
use agent_engine::NativeEngine;
use agent_engine::lsp::fake::ScriptedServer;
use agent_engine::lsp::{LanguageServer, LspPool, Starter};
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;

use crate::{Source, finish, model, start};

fn engine(models: Vec<Arc<ScriptedModel>>) -> NativeEngine {
    NativeEngine::new(Arc::new(Source(Mutex::new(models))), Arc::new(InMemorySessions::default()))
}

fn offered(model: &ScriptedModel) -> Vec<String> {
    model.asked()[0].tools.iter().map(|tool| tool.name.clone()).collect()
}

fn results(model: &ScriptedModel) -> String {
    format!("{:?}", model.asked().last().expect("asked").messages)
}

#[test]
fn the_agent_asks_the_file_s_language_server() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.rs"), "struct Grep;\n").expect("file");
    let server = ScriptedServer::answering([Ok(json!([{"name": "Grep", "kind": 23,
        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 11}}}]))]);
    let agent = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "l".into(),
            name: "LSP".into(),
            input: json!({"operation": "documentSymbol", "file_path": "a.rs"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Found it.".into())]),
    ]));
    let engine = engine(vec![agent.clone()]).with_language_servers(server.lsp().servers);
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "outline a.rs", model())));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert!(offered(&agent).contains(&"LSP".to_owned()));
    assert!(results(&agent).contains("struct Grep (line 1)"), "{}", results(&agent));
}

#[test]
fn servers_the_settings_disable_are_not_offered() {
    let dir = tempfile::tempdir().expect("dir");
    let off: serde_json::Map<String, serde_json::Value> = ["rust-analyzer", "typescript", "pyright", "gopls", "clangd"]
        .iter()
        .map(|n| (n.to_string(), json!({"disabled": true})))
        .collect();
    std::fs::create_dir_all(dir.path().join(".superfik")).expect("dir");
    std::fs::write(dir.path().join(".superfik/settings.json"), json!({"lspServers": off}).to_string())
        .expect("settings");
    let agent = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hi".into())])]));
    let engine = engine(vec![agent.clone()]).with_language_servers(ScriptedServer::answering([]).lsp().servers);
    finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "hi", model())));
    assert!(!offered(&agent).contains(&"LSP".to_owned()), "{:?}", offered(&agent));
}

struct NothingInstalled;

impl Starter for NothingInstalled {
    fn find(&self, _: &str) -> Option<PathBuf> {
        None
    }
    fn start(&self, _: &ServerSpec, _: &Path, _: &Path, _: &AtomicBool) -> Result<Arc<dyn LanguageServer>, String> {
        Err("not installed".into())
    }
}

#[test]
fn without_an_installed_server_the_tool_is_not_offered() {
    let dir = tempfile::tempdir().expect("dir");
    let agent = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hi".into())])]));
    let pool = Arc::new(LspPool::new(NothingInstalled, Duration::from_secs(60)));
    let engine = engine(vec![agent.clone()]).with_language_servers(pool);
    finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "hi", model())));
    assert!(!offered(&agent).contains(&"LSP".to_owned()), "{:?}", offered(&agent));
}
