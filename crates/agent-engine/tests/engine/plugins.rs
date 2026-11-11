use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, Extensions, PromptRequest};

use crate::{Source, finish, model, start};

pub(crate) const SERVER: &str = r#"while IFS= read -r line; do
case "$line" in
*'"method":"initialize"'*) printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}';;
*'"method":"tools/list"'*) printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"ping","inputSchema":{"type":"object"}}]}}';;
esac
done"#;

#[test]
#[cfg(unix)]
fn a_plugin_brings_its_command_hook_and_mcp_tools_to_a_run() {
    let repo = tempfile::tempdir().expect("repo");
    let plugin = tempfile::tempdir().expect("plugin");
    let root = plugin.path();
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("write");
    };
    write(concat!(brand::plugin_dir!(), "/plugin.json"), r#"{"name": "acme"}"#);
    write("commands/hello.md", "---\ndescription: Greets\n---\nPLUGIN_COMMAND_SENTINEL $ARGUMENTS");
    write("server.sh", SERVER);
    write(
        ".mcp.json",
        concat!(
            r#"{"mcpServers": {"tools": {"command": "/bin/sh", "args": ["${"#,
            brand::env_prefix!(),
            r#"_PLUGIN_ROOT}/server.sh"]}}}"#
        ),
    );
    write(
        "hooks/hooks.json",
        concat!(
            r#"{"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": "touch '${"#,
            brand::env_prefix!(),
            r#"_PLUGIN_ROOT}/fired'"}]}]}}"#
        ),
    );

    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hi".into())])]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(repo.path().into(), "/acme:hello world", model());
    request.extensions = Some(Extensions { plugins: vec![root.to_path_buf()], ..Extensions::default() });
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");

    let asked = &model_api.asked()[0];
    assert!(asked.messages.last().expect("user").text().contains("PLUGIN_COMMAND_SENTINEL world"));
    let tools: Vec<&str> = asked.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(tools.contains(&"mcp__plugin_acme_tools__ping"), "{tools:?}");
    assert!(root.join("fired").exists(), "the plugin's hook ran");
}
