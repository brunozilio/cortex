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
fn an_mcp_sign_in_is_refused_in_the_sandbox_and_asked_about_outside_it() {
    let login = format!("{} mcp login jira", brand::COMMAND);
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    rig.workspace.policy.sandbox = Some(domain::sandbox::Sandbox::default());
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("in", "Bash", json!({ "command": login }))]),
        ScriptedModel::reply(vec![tool_use("out", "Bash", json!({ "command": login, "unsandboxed": true }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    let answerer = answering(&rig, Decision::Deny);
    rig.run(&model, &mut vec![Message::user("sign in")]).expect("run");
    answerer.join().expect("answerer");

    let events = rig.events();
    let refused_inside = events
        .iter()
        .any(|event| matches!(event, EngineEvent::Refused { reason, .. } if reason.contains("/mcp login jira")));
    let asked = events.iter().filter(|event| matches!(event, EngineEvent::Asked(_))).count();
    if platform::sandbox::supported() {
        assert!(refused_inside, "the sandboxed run cannot keep the sign-in, so it never starts");
        assert_eq!(asked, 1, "only the run outside the sandbox is asked about");
    } else {
        assert!(!refused_inside, "without a sandbox nothing is confined");
    }
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
    if platform::sandbox::supported() {
        assert!(asked.is_some_and(|subject| subject.contains("OUTSIDE the sandbox")));
        assert!(!rig.dir.path().join("made.txt").exists(), "a denied call never runs");
    } else {
        assert!(asked.is_none(), "without a sandbox there is nothing to leave");
    }
}

struct NoSandboxHere;

impl agent_engine::tools::Launcher for NoSandboxHere {
    fn launch(
        &self,
        command: std::process::Command,
        _: Option<&platform::sandbox::Confinement>,
    ) -> std::process::Command {
        command
    }

    fn support(&self) -> platform::sandbox::Support {
        platform::sandbox::Support::Missing("this system has no sandbox for commands".into())
    }
}

#[test]
fn a_required_sandbox_this_system_cannot_provide_refuses_commands_instead_of_running_them_unconfined() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.sandbox = Some(domain::sandbox::Sandbox { required: true, ..Default::default() });
    rig.workspace.services.launch = agent_engine::tools::Launch(Arc::new(NoSandboxHere));
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": "echo out > made.txt" }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    assert!(!rig.dir.path().join("made.txt").exists());
    let refused = rig.events().into_iter().find_map(|event| match event {
        EngineEvent::Refused { reason, .. } => Some(reason),
        _ => None,
    });
    assert!(
        refused.as_deref().is_some_and(|why| why.contains("sandbox.required") && why.contains("no sandbox")),
        "{refused:?}"
    );
}

#[test]
fn by_default_a_sandbox_this_system_cannot_provide_refuses_commands_where_the_app_has_one() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.sandbox = Some(domain::sandbox::Sandbox::default());
    rig.workspace.services.launch = agent_engine::tools::Launch(Arc::new(NoSandboxHere));
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": "echo out > made.txt" }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    let made = rig.dir.path().join("made.txt").exists();
    assert_eq!(made, !domain::sandbox::REQUIRED_BY_DEFAULT, "required by default on Linux and macOS only");
    if domain::sandbox::REQUIRED_BY_DEFAULT {
        let refused = rig.events().into_iter().find_map(|event| match event {
            EngineEvent::Refused { reason, .. } => Some(reason),
            _ => None,
        });
        assert!(refused.as_deref().is_some_and(|why| why.contains(r#""required": false"#)), "{refused:?}");
    }
}

#[test]
fn a_sandbox_this_system_cannot_provide_lets_commands_run_unconfined_when_not_required() {
    let mut rig = Rig::new(PermissionMode::Bypass);
    rig.workspace.policy.sandbox = Some(domain::sandbox::Sandbox { required: false, ..Default::default() });
    rig.workspace.services.launch = agent_engine::tools::Launch(Arc::new(NoSandboxHere));
    let model = ScriptedModel::new([
        ScriptedModel::reply(vec![tool_use("b", "Bash", json!({ "command": "echo out > made.txt" }))]),
        ScriptedModel::reply(vec![Block::Text("Done".into())]),
    ]);
    rig.run(&model, &mut vec![Message::user("go")]).expect("run");
    assert!(rig.dir.path().join("made.txt").exists(), "the notice at the start of the run says it is unconfined");
}
