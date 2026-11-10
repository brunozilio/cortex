//! A project's `.superfik/mcp.json` servers reach a run once the owner approved them.

use std::sync::{Arc, Mutex};

use superfik_core::{EngineEvent, PromptRequest};
use superfik_native::NativeEngine;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::sessions::InMemorySessions;
use superfik_native::transcript::Block;

use crate::plugins::SERVER;
use crate::{Source, finish, model, start};

fn offered(repo: &std::path::Path) -> Vec<String> {
    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hi".into())])]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &PromptRequest::plain(repo.into(), "hi", model())));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    model_api.asked()[0].tools.iter().map(|tool| tool.name.clone()).collect()
}

#[test]
#[cfg(unix)]
fn a_project_server_reaches_a_run_once_approved() {
    let repo = tempfile::tempdir().expect("repo");
    std::fs::write(repo.path().join("server.sh"), SERVER).expect("server");
    let config = format!(
        r#"{{"mcpServers": {{"jira": {{"command": "/bin/sh", "args": ["{}"]}}}}}}"#,
        repo.path().join("server.sh").display()
    );
    std::fs::create_dir_all(repo.path().join(".superfik")).expect("dir");
    std::fs::write(repo.path().join(".superfik/mcp.json"), config).expect("config");
    assert!(!offered(repo.path()).iter().any(|name| name.starts_with("mcp__jira__")), "not approved yet");

    std::fs::write(repo.path().join(".superfik/settings.local.json"), r#"{"enabledMcpServers": ["jira"]}"#)
        .expect("approve");
    assert!(offered(repo.path()).contains(&"mcp__jira__ping".to_owned()));
}
