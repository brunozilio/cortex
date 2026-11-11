use std::path::Path;
use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, Extensions, PromptRequest};

use crate::plugins::SERVER;
use crate::{Source, finish, model, start};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, text).expect("write");
}

#[test]
#[cfg(unix)]
fn nothing_of_claude_code_s_folders_reaches_a_run() {
    let repo = tempfile::tempdir().expect("repo");
    let home = tempfile::tempdir().expect("home");
    let (r, h) = (repo.path(), home.path());
    write(&r.join("CLAUDE.md"), "SENTINEL_ROOT_CLAUDE_MD");
    write(&r.join("CLAUDE.local.md"), "SENTINEL_LOCAL_CLAUDE_MD");
    write(&r.join(".claude/CLAUDE.md"), "SENTINEL_DOT_CLAUDE_MD");
    write(&r.join(".claude/rules/all.md"), "SENTINEL_CLAUDE_RULE");
    write(&r.join(".claude/skills/sentinel-skill/SKILL.md"), "---\nname: sentinel-skill\ndescription: S\n---\nS");
    write(&r.join(".claude/agents/sentinel-agent.md"), "---\nname: sentinel-agent\ndescription: A\n---\nA");
    write(&r.join(".claude/commands/sentinel.md"), "---\ndescription: C\n---\nSENTINEL_CLAUDE_COMMAND");
    write(
        &r.join(".claude/settings.json"),
        r#"{"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": "touch fired"}]}]}}"#,
    );
    write(&r.join("server.sh"), SERVER);
    write(
        &r.join(".mcp.json"),
        &format!(
            r#"{{"mcpServers": {{"jira": {{"command": "/bin/sh", "args": ["{}"]}}}}}}"#,
            r.join("server.sh").display()
        ),
    );
    write(&r.join(concat!(brand::dir!(), "/settings.local.json")), r#"{"enabledMcpServers": ["jira"]}"#);
    write(&h.join(".claude/CLAUDE.md"), "SENTINEL_HOME_CLAUDE_MD");
    write(&h.join(".claude/rules/all.md"), "SENTINEL_HOME_CLAUDE_RULE");
    write(&h.join(".claude/skills/home-sentinel/SKILL.md"), "---\nname: home-sentinel\ndescription: S\n---\nS");

    let model_api = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hi".into())])]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model_api.clone()]))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(r.into(), "/sentinel go", model());
    request.extensions = Some(Extensions { home: Some(h.to_path_buf()), ..Extensions::default() });
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");

    let asked = &model_api.asked()[0];
    let seen = format!("{}\n{:?}\n{:?}", asked.system, asked.tools, asked.messages);
    for sentinel in [
        "SENTINEL_ROOT_CLAUDE_MD",
        "SENTINEL_LOCAL_CLAUDE_MD",
        "SENTINEL_DOT_CLAUDE_MD",
        "SENTINEL_CLAUDE_RULE",
        "SENTINEL_HOME_CLAUDE_MD",
        "SENTINEL_HOME_CLAUDE_RULE",
        "sentinel-skill",
        "home-sentinel",
        "sentinel-agent",
        "SENTINEL_CLAUDE_COMMAND",
        "mcp__jira__",
    ] {
        assert!(!seen.contains(sentinel), "{sentinel} reached the run");
    }
    assert!(!r.join("fired").exists(), ".claude/settings.json hooks do not run");
}
