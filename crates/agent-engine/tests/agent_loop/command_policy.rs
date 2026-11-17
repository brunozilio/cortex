use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn bash(id: &str, command: &str) -> Block {
    tool_use(id, "Bash", json!({ "command": command }))
}

fn denying(rig: &Rig) -> std::thread::JoinHandle<()> {
    let control = Arc::clone(&rig.control);
    let events = Arc::clone(&rig.events);
    std::thread::spawn(move || {
        for _ in 0..200 {
            let asks: Vec<_> = events
                .lock()
                .expect("events")
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            for id in asks {
                control.answer(&id, Decision::Deny);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    })
}

fn results(history: &[Message]) -> String {
    format!("{:?}", history.iter().flat_map(|m| &m.blocks).collect::<Vec<_>>())
}

#[test]
fn read_only_commands_run_without_asking_even_while_planning() {
    let rig = Rig::new(PermissionMode::Plan);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![bash("b1", "ls && cat notes.txt")]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let mut history = vec![Message::user("look")];
    rig.run(&model, &mut history).expect("run");

    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_))));
    assert!(results(&history).contains("beta"), "the command ran: {}", results(&history));
}

#[test]
fn a_reader_behind_a_changed_environment_a_wrapper_or_a_wildcard_does_not_run_while_planning() {
    for command in ["PATH=./bin ls", "env cat notes.txt", "cat note?.txt", "sort -o stolen.txt notes.txt"] {
        let rig = Rig::new(PermissionMode::Plan);
        let model = ScriptedModel::new([
            ScriptedModel::reply(vec![bash("b1", command)]),
            ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
        ]);
        let mut history = vec![Message::user("look")];
        rig.run(&model, &mut history).expect("run");

        assert!(!results(&history).contains("beta"), "{command} ran: {}", results(&history));
        assert!(!rig.dir.path().join("stolen.txt").exists(), "{command}");
        assert!(
            rig.events().iter().any(|event| matches!(event, EngineEvent::Refused { tool, .. } if tool == "Bash")),
            "{command}"
        );
    }
}

#[test]
fn a_wildcard_that_could_print_a_secret_file_is_refused_even_when_bypassing() {
    let rig = Rig::new(PermissionMode::Bypass);
    std::fs::write(rig.dir.path().join(".env"), "API_KEY=sk-live-123").expect("write");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![bash("b1", "cat .env*")]),
        ScriptedModel::reply(vec![bash("b2", "cat .en?")]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let mut history = vec![Message::user("show config")];
    rig.run(&model, &mut history).expect("run");
    assert!(!results(&history).contains("sk-live-123"), "{}", results(&history));
}

#[test]
fn printing_secrets_is_refused_even_when_bypassing() {
    let rig = Rig::new(PermissionMode::Bypass);
    std::fs::write(rig.dir.path().join(".env"), "API_KEY=sk-live-123").expect("write");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![bash("b1", "cat .env")]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let mut history = vec![Message::user("show config")];
    rig.run(&model, &mut history).expect("run");

    assert!(!results(&history).contains("sk-live-123"));
    assert!(rig.events().iter().any(|event| matches!(
        event,
        EngineEvent::Refused { tool, reason } if tool == "Bash" && reason.contains(".env")
    )));
}

#[test]
fn a_force_push_is_asked_about_even_when_bypassing() {
    let rig = Rig::new(PermissionMode::Bypass);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![bash("b1", "git push --force > pushed.txt")]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let answerer = denying(&rig);
    rig.run(&model, &mut vec![Message::user("ship it")]).expect("run");
    answerer.join().expect("answerer");

    assert!(!rig.dir.path().join("pushed.txt").exists(), "a denied push never runs");
    assert!(rig.events().iter().any(|event| matches!(
        event,
        EngineEvent::Asked(ask) if matches!(&ask.kind, AskKind::Permission { tool, .. } if tool == "Bash")
    )));
}
