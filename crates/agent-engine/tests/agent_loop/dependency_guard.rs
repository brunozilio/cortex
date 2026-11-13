use std::sync::Arc;

use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::tools::Network;
use agent_engine::transcript::{Block, Message};
use domain::{AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn install(rig: &Rig, package: &str) {
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use(
            "b",
            "Bash",
            json!({ "command": format!("true || npm install {package}") }),
        )]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let control = Arc::clone(&rig.control);
    let events = Arc::clone(&rig.events);
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
                control.answer(&id, Decision::Deny);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    rig.run(&model, &mut vec![Message::user("install")]).expect("run");
    answerer.join().expect("answerer");
}

fn guarded(answers: Vec<(u16, String)>) -> (Rig, Arc<ScriptedHttp>) {
    let mut rig = Rig::new(PermissionMode::Bypass);
    let http = Arc::new(ScriptedHttp::new(answers));
    rig.workspace.services.network = Some(Network(Arc::clone(&http) as Arc<dyn agent_engine::model::http::Http>));
    rig.workspace.policy.dependency_guard = true;
    (rig, http)
}

#[test]
fn a_made_up_package_is_asked_about_even_in_bypass() {
    let (rig, http) = guarded(vec![(404, "{}".into())]);

    install(&rig, "react-super-utils-pro");

    let asked = rig.events().into_iter().find_map(|event| match event {
        EngineEvent::Asked(ask) => match ask.kind {
            AskKind::Permission { subject, .. } => Some(subject),
            _ => None,
        },
        _ => None,
    });
    assert!(asked.is_some_and(|subject| subject.contains("not published")), "the user is told why");
    assert!(rig.events().iter().any(|event| matches!(event, EngineEvent::Refused { tool, .. } if tool == "Bash")));
    assert_eq!(http.sent()[0].url, "https://registry.npmjs.org/react-super-utils-pro");
}

#[test]
fn a_well_known_package_installs_without_asking_in_bypass() {
    let (rig, _) = guarded(vec![(200, r#"{"time":{"created":"2011-01-01T00:00:00Z"}}"#.into())]);

    install(&rig, "express-session");

    assert!(!rig.events().iter().any(|event| matches!(event, EngineEvent::Asked(_))));
}
