use std::sync::Arc;

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn answering(rig: &Rig, decision: Decision) -> std::thread::JoinHandle<()> {
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
                control.answer(&id, decision.clone());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    })
}

#[test]
fn leaving_the_sandbox_is_always_asked_even_in_bypass() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.sandbox = Some(domain::sandbox::Sandbox::default());
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "b",
            "Bash",
            json!({ "command": "echo out > made.txt", "unsandboxed": true }),
        )]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let answerer = answering(&rig, Decision::Deny);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    answerer.join().expect("answerer");

    let asked = rig.events().into_iter().find_map(|event| match event {
        EngineEvent::Asked(ask) => match ask.kind {
            AskKind::Permission { subject, .. } => Some(subject),
            _ => None,
        },
        _ => None,
    });
    if platform::sandbox::SUPPORTED {
        assert!(asked.is_some_and(|subject| subject.contains("OUTSIDE the sandbox")));
        assert!(!rig.dir.path().join("made.txt").exists(), "a denied call never runs");
    } else {
        assert!(asked.is_none(), "without a sandbox there is nothing to leave");
    }
}
