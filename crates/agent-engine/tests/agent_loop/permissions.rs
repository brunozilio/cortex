//! Asking the user: permissions for commands and outside reads, plan mode,
//! and questions on the card.

use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

#[test]
fn asks_before_commands_and_honours_the_answer() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b1", "Bash", json!({"command": "echo hi > out.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let control = Arc::clone(&rig.control);
    let pending_events = Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
        for _ in 0..200 {
            let asks: Vec<_> = pending_events
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
    });
    rig.run(&model, &mut vec![Message::user("write")]).expect("run");
    answerer.join().expect("answerer");

    assert!(!rig.dir.path().join("out.txt").exists(), "a denied command never runs");
    let events = rig.events();
    assert!(events.iter().any(|event| matches!(
        event,
        EngineEvent::Asked(ask) if matches!(&ask.kind, AskKind::Permission { tool, subject, .. } if tool == "Bash" && subject == "echo hi > out.txt")
    )));
    assert!(events.iter().any(|event| matches!(event, EngineEvent::ToolResult { failed: true, .. })));
}

#[test]
fn reading_outside_the_project_is_asked_for_by_path_and_a_denial_keeps_it_unread() {
    let rig = Rig::new(PermissionMode::Plan);
    let elsewhere = tempfile::tempdir().expect("dir");
    let secret = elsewhere.path().join("id_ed25519");
    std::fs::write(&secret, "PRIVATE KEY").expect("write");
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![
            tool_use("in", "Read", json!({"file_path": "notes.txt"})),
            tool_use("out", "Read", json!({"file_path": secret.display().to_string()})),
            tool_use("grep", "Grep", json!({"pattern": "KEY", "path": elsewhere.path().display().to_string()})),
        ]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let control = Arc::clone(&rig.control);
    let pending_events = Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
        for _ in 0..200 {
            let asks: Vec<_> = pending_events
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
    });
    rig.run(&model, &mut vec![Message::user("look")]).expect("run");
    answerer.join().expect("answerer");

    let asked: Vec<(String, String)> = rig
        .events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::Asked(ask) => match ask.kind {
                AskKind::Permission { tool, subject, .. } => Some((tool, subject)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 2, "only the reads outside: {asked:?}");
    assert!(asked.iter().any(|(tool, subject)| tool == "Read" && subject.ends_with("id_ed25519")));
    assert!(asked.iter().any(|(tool, _)| tool == "Grep"));
    let results = &model.asked()[1].messages[2].blocks;
    let text = format!("{results:?}");
    assert!(text.contains("alpha"), "the project's file was read");
    assert!(!text.contains("PRIVATE KEY"), "the outside file was not");
}

#[test]
fn plan_mode_refuses_edits_without_asking() {
    let rig = Rig::new(PermissionMode::Plan);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("w", "Write", json!({"file_path": "new.txt", "content": "x"}))]),
        ScriptedModel::reply(vec![Block::Text("Plan: …".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("plan it")]).expect("run");
    assert!(!rig.dir.path().join("new.txt").exists());
    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_))));
}

#[test]
fn asks_the_user_questions_on_the_card_even_while_planning() {
    let rig = Rig::new(PermissionMode::Plan);
    let question = json!({ "questions": [{
        "question": "Which database?", "header": "Database", "multiSelect": false,
        "options": [{ "label": "SQLite", "description": "One file." }, { "label": "Postgres", "description": "A server." }],
    }] });
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("q1", "AskUserQuestion", question)]),
        ScriptedModel::reply(vec![Block::Text("Postgres it is.".into())]),
    ]);
    let control = Arc::clone(&rig.control);
    let pending_events = Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
        for _ in 0..200 {
            let asks: Vec<_> = pending_events
                .lock()
                .expect("events")
                .iter()
                .filter_map(|event| match event {
                    EngineEvent::Asked(ask) => Some(ask.id.clone()),
                    _ => None,
                })
                .collect();
            for id in asks {
                control.answer(&id, Decision::Answer(vec![vec![1]]));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let mut history = vec![Message::user("set up storage")];
    rig.run(&model, &mut history).expect("run");
    answerer.join().expect("answerer");

    let asked = rig.events().into_iter().find_map(|event| match event {
        EngineEvent::Asked(ask) => Some(ask.kind),
        _ => None,
    });
    assert!(matches!(asked, Some(AskKind::Questions(ref q)) if q[0].options[1].label == "Postgres"), "{asked:?}");
    let result = history.iter().flat_map(|m| &m.blocks).find_map(|block| match block {
        Block::ToolResult { content, is_error: false, .. } => Some(content.clone()),
        _ => None,
    });
    assert_eq!(
        result.as_deref(),
        Some("The user answered: \"Which database?\" = \"Postgres\". Go on with these answers in mind.")
    );
}
