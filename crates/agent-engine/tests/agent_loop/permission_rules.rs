use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::permission_rules::{PermissionRules, RuleKind};
use domain::{EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn rig(mode: PermissionMode, rules: PermissionRules) -> Rig {
    let mut rig = Rig::new(mode);
    rig.workspace.permissions = rules;
    rig
}

fn one_call(name: &str, input: serde_json::Value) -> ScriptedModel {
    ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("c", name, input)]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ])
}

fn asked(rig: &Rig) -> bool {
    rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_)))
}

#[test]
fn an_allowed_command_runs_without_asking() {
    let rig = rig(PermissionMode::Ask, PermissionRules::default().with(RuleKind::Allow, "Bash(touch:*)"));
    let model = one_call("Bash", json!({"command": "touch made.txt"}));
    rig.run(&model, &mut vec![Message::user("make it")]).expect("run");
    assert!(!asked(&rig));
    assert!(rig.dir.path().join("made.txt").exists());
}

#[test]
fn a_denied_command_is_refused_even_when_bypassing() {
    let rig = rig(PermissionMode::Bypass, PermissionRules::default().with(RuleKind::Deny, "Bash(rm:*)"));
    let model = one_call("Bash", json!({"command": "rm notes.txt"}));
    rig.run(&model, &mut vec![Message::user("clean up")]).expect("run");
    assert!(rig.dir.path().join("notes.txt").exists(), "a denied command never runs");
    let result = model.asked()[1].messages.last().cloned().expect("results");
    assert!(
        matches!(&result.blocks[0], Block::ToolResult { content, is_error: true, .. } if content.contains("Bash(rm:*)")),
        "the refusal names the rule: {result:?}"
    );
}

#[test]
fn an_ask_rule_asks_even_for_edits_the_mode_allows() {
    let rig = rig(PermissionMode::AcceptEdits, PermissionRules::default().with(RuleKind::Ask, "Edit(notes.txt)"));
    let control = std::sync::Arc::clone(&rig.control);
    let events = std::sync::Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
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
                control.answer(&id, domain::Decision::Deny);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let model = one_call("Write", json!({"file_path": "notes.txt", "content": "gone"}));
    rig.run(&model, &mut vec![Message::user("rewrite")]).expect("run");
    answerer.join().expect("answerer");
    assert!(asked(&rig));
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("notes.txt")).expect("read"), "alpha\nbeta\n");
}

#[test]
fn a_hook_allowing_the_call_does_not_skip_an_ask_rule() {
    let mut rig = rig(PermissionMode::Ask, PermissionRules::default().with(RuleKind::Ask, "Bash(touch:*)"));
    let allow = r#"echo '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"}}'"#;
    rig.workspace.hooks = agent_engine::hooks::Hooks::parse(
        &json!({"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command": allow}]}]}),
    )
    .expect("hooks");
    let control = std::sync::Arc::clone(&rig.control);
    let events = std::sync::Arc::clone(&rig.events);
    let answerer = std::thread::spawn(move || {
        for _ in 0..300 {
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
                control.answer(&id, domain::Decision::Deny);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let model = one_call("Bash", json!({"command": "touch made.txt"}));
    rig.run(&model, &mut vec![Message::user("make it")]).expect("run");
    answerer.join().expect("answerer");
    assert!(asked(&rig), "the ask rule still asks");
    assert!(!rig.dir.path().join("made.txt").exists());
}

fn answering(rig: &Rig, decision: domain::Decision) -> std::thread::JoinHandle<()> {
    let control = std::sync::Arc::clone(&rig.control);
    let events = std::sync::Arc::clone(&rig.events);
    std::thread::spawn(move || {
        for _ in 0..300 {
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
                control.answer(&id, decision.clone());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    })
}

#[test]
fn always_keeps_the_suggested_rule_in_the_project_and_stops_asking() {
    let rig = Rig::new(PermissionMode::Ask);
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("a", "Bash", json!({"command": "touch a.txt"}))]),
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({"command": "touch b.txt"}))]),
        ScriptedModel::reply(vec![Block::Text("Ok.".into())]),
    ]);
    let answerer = answering(&rig, domain::Decision::AllowAlways);
    rig.run(&model, &mut vec![Message::user("make them")]).expect("run");
    answerer.join().expect("answerer");
    let asks: Vec<_> = rig.events().into_iter().filter(|event| matches!(event, EngineEvent::Asked(_))).collect();
    assert!(
        matches!(&asks[..], [EngineEvent::Asked(ask)] if matches!(&ask.kind, domain::AskKind::Permission { rule: Some(rule), .. } if rule == "Bash(touch:*)")),
        "asked once, offering the rule: {asks:?}"
    );
    assert!(rig.dir.path().join("a.txt").exists() && rig.dir.path().join("b.txt").exists());
    let local =
        std::fs::read_to_string(rig.dir.path().join(concat!(brand::dir!(), "/settings.local.json"))).expect("kept");
    assert!(local.contains("Bash(touch:*)"), "{local}");
}

#[test]
fn asking_runs_the_notification_hooks() {
    let mut rig = Rig::new(PermissionMode::Ask);
    rig.workspace.hooks = agent_engine::hooks::Hooks::parse(&json!({"Notification": [
        {"matcher": "permission_prompt", "hooks": [{"type": "command", "command": "cat > notified.json"}]}
    ]}))
    .expect("hooks");
    let model = one_call("Bash", json!({"command": "touch a.txt"}));
    let answerer = answering(&rig, domain::Decision::Deny);
    rig.run(&model, &mut vec![Message::user("make it")]).expect("run");
    answerer.join().expect("answerer");
    let notified: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(rig.dir.path().join("notified.json")).expect("notified"))
            .expect("json");
    assert_eq!(notified["hook_event_name"], "Notification");
    assert_eq!(notified["message"], concat!(brand::name!(), " needs your permission to use Bash"));
}

#[test]
fn a_refusal_with_a_note_tells_the_agent_what_to_do() {
    let rig = Rig::new(PermissionMode::Ask);
    let model = one_call("Bash", json!({"command": "touch a.txt"}));
    let answerer = answering(&rig, domain::Decision::DenyWith("write it with Write".into()));
    rig.run(&model, &mut vec![Message::user("make it")]).expect("run");
    answerer.join().expect("answerer");
    let result = model.asked()[1].messages.last().cloned().expect("results");
    assert!(
        matches!(&result.blocks[0], Block::ToolResult { content, is_error: true, .. } if content.contains("write it with Write")),
        "{result:?}"
    );
}

#[test]
fn a_denied_read_is_refused_inside_the_project() {
    let rig = rig(PermissionMode::AcceptEdits, PermissionRules::default().with(RuleKind::Deny, "Read(*.txt)"));
    let model = one_call("Read", json!({"file_path": "notes.txt"}));
    rig.run(&model, &mut vec![Message::user("read")]).expect("run");
    let result = model.asked()[1].messages.last().cloned().expect("results");
    assert!(matches!(&result.blocks[0], Block::ToolResult { is_error: true, .. }), "{result:?}");
}
