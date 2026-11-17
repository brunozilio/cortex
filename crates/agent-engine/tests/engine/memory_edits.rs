use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::{Block, Role};
use domain::memory::MemoryBrief;
use domain::{EngineEvent, PromptRequest};

use crate::{Source, finish, model, start};

fn asked_with(output: &str) -> agent_engine::model::TurnRequest {
    asked_in(&tempfile::tempdir().expect("dir"), output)
}

fn asked_in(dir: &tempfile::TempDir, output: &str) -> agent_engine::model::TurnRequest {
    let agent = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Listed.".into())])]));
    let engine = Engine::new(Arc::new(Source(Mutex::new(vec![agent.clone()]))), Arc::new(InMemorySessions::default()));
    let brief = MemoryBrief { output: output.into(), ..MemoryBrief::default() };
    let mut request = PromptRequest::plain(dir.path().into(), "list the issues", model());
    request.host.memory = Some(brief);
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    agent.asked().first().cloned().expect("asked")
}

#[test]
fn the_user_s_words_stay_alone_and_the_memory_file_is_a_short_reminder() {
    let asked = asked_with("/tmp/edits-1.json");
    let user = asked.messages.iter().find(|message| message.role == Role::User).expect("user");

    let texts: Vec<&str> =
        user.blocks.iter().filter_map(|b| if let Block::Text(text) = b { Some(text.as_str()) } else { None }).collect();
    assert_eq!(texts, ["list the issues"]);
    let reminders: String = user
        .blocks
        .iter()
        .filter_map(|b| if let Block::Reminder(text) = b { Some(text.as_str()) } else { None })
        .collect();
    assert!(reminders.contains("/tmp/edits-1.json"), "{reminders}");
    assert!(!reminders.contains("\"reclassify\""), "the format is not repeated every turn: {reminders}");
}

#[test]
fn the_memory_format_is_in_the_system_prompt_and_the_same_from_run_to_run() {
    let dir = tempfile::tempdir().expect("dir");
    let first = asked_in(&dir, "/tmp/edits-1.json");
    let second = asked_in(&dir, "/tmp/edits-2.json");

    assert!(first.system.contains("\"reclassify\""), "{}", first.system);
    assert!(!first.system.contains("/tmp/edits-1.json"), "a run's own file would break the prompt cache");
    assert_eq!(first.system, second.system);
}
