use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::memory::{Author, Memory, MemoryBrief, MemoryId, MemoryKind};
use domain::{EngineEvent, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start};

fn retries() -> Memory {
    Memory {
        id: MemoryId(7),
        kind: MemoryKind::Semantic,
        text: "Keep defaultRetryMax at 4".into(),
        tags: Vec::new(),
        created: 0,
        origin: None,
        author: Author::User,
        history: Vec::new(),
        uses: 0,
        last_used: 0,
        touches: vec!["client.go".into()],
        credit: Some("Maya, 2026-10-09".into()),
    }
}

fn reading_twice() -> Arc<ScriptedModel> {
    let read =
        |id: &str| Block::ToolUse { id: id.into(), name: "Read".into(), input: json!({"file_path": "client.go"}) };
    Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![read("one")]),
        ScriptedModel::reply(vec![read("two")]),
        ScriptedModel::reply(vec![Block::Text("Read it.".into())]),
    ]))
}

fn run(anchors: bool) -> String {
    run_in(tempfile::tempdir().expect("dir"), anchors)
}

fn run_in(dir: tempfile::TempDir, anchors: bool) -> String {
    std::fs::write(dir.path().join("client.go"), "retries = 4\n").expect("file");
    let agent = reading_twice();
    let engine = Engine::new(Arc::new(Source(Mutex::new(vec![agent.clone()]))), Arc::new(InMemorySessions::default()));
    let brief = MemoryBrief { anchored: vec![retries()], anchors, ..MemoryBrief::default() };
    let mut request = PromptRequest::plain(dir.path().into(), "look at client.go", model());
    request.host.memory = Some(brief);
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    format!("{:?}", agent.asked().last().expect("asked").messages)
}

#[test]
fn reading_a_file_a_memory_is_about_shows_that_memory_once() {
    let seen = run(true);

    assert_eq!(seen.matches("Keep defaultRetryMax at 4 (Maya, 2026-10-09)").count(), 1, "{seen}");
    assert!(seen.contains("Kept from earlier sessions about client.go"), "{seen}");
}

#[test]
fn without_anchors_the_file_is_read_alone() {
    assert!(!run(false).contains("Keep defaultRetryMax"));
}

#[test]
fn a_project_that_turns_anchors_off_reads_the_file_alone_even_with_a_brief() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join(brand::DIR)).expect("settings folder");
    let off = json!({ "features": { domain::features::MEMORY_ANCHORS: false } });
    std::fs::write(dir.path().join(concat!(brand::dir!(), "/settings.json")), off.to_string()).expect("settings");

    assert!(!run_in(dir, true).contains("Keep defaultRetryMax"));
}
