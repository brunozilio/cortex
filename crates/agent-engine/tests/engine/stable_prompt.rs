use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{EngineEvent, PromptRequest};

use crate::{Source, finish, model, start};

fn git(dir: &Path, args: &[&str]) {
    let done = Command::new("git").args(args).current_dir(dir).output().expect("git");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("dir");
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "start"]);
    dir
}

#[test]
fn later_prompts_of_a_session_send_the_system_prompt_of_its_first() {
    let dir = repository();
    let first = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("One.".into())])]));
    let second = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Two.".into())])]));
    let fresh = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Three.".into())])]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&first), Arc::clone(&second), Arc::clone(&fresh)]))),
        Arc::new(InMemorySessions::default()),
    );
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "one", model())));
    let Some(EngineEvent::SessionStarted(session)) = events.first().cloned() else { panic!("{events:?}") };

    std::fs::write(dir.path().join("edited.rs"), "fn main() {}").expect("edit");
    let mut again = PromptRequest::plain(dir.path().into(), "two", model());
    again.resume = Some(session);
    finish(&start(&engine, &again));
    assert_eq!(second.asked()[0].system, first.asked()[0].system, "the cached prefix survives the edit");

    finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "three", model())));
    assert!(fresh.asked()[0].system.contains("edited.rs"), "a new session sees the repository as it is now");
}
