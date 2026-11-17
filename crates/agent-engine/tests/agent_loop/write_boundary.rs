use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::{Block, Message};
use domain::permission_rules::{PermissionRules, RuleKind};
use domain::{AskId, AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::json;

use crate::{Rig, tool_use};

fn writes(paths: &[&Path]) -> ScriptedModel {
    let calls = paths
        .iter()
        .enumerate()
        .map(|(at, path)| {
            tool_use(&format!("w{at}"), "Write", json!({"file_path": path.display().to_string(), "content": "x"}))
        })
        .map(|call| ScriptedModel::reply(vec![call]));
    ScriptedModel::new(calls.chain([ScriptedModel::reply(vec![Block::Text("Ok.".into())])]))
}

fn ask_ids(rig_events: &std::sync::Mutex<Vec<EngineEvent>>) -> Vec<AskId> {
    let events = rig_events.lock().expect("events");
    events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::Asked(ask) => Some(ask.id.clone()),
            _ => None,
        })
        .collect()
}

fn run_answering(rig: &Rig, model: &ScriptedModel, answers: Vec<Decision>) -> Vec<String> {
    let done = Arc::new(AtomicBool::new(false));
    let (control, events, finished) = (Arc::clone(&rig.control), Arc::clone(&rig.events), Arc::clone(&done));
    let answerer = std::thread::spawn(move || {
        let mut answered = 0;
        while !finished.load(Ordering::SeqCst) {
            for id in ask_ids(&events).iter().skip(answered) {
                control.answer(id, answers.get(answered).cloned().unwrap_or(Decision::Deny));
                answered += 1;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    let ran = rig.run(model, &mut vec![Message::user("write")]);
    done.store(true, Ordering::SeqCst);
    answerer.join().expect("answerer");
    ran.expect("run");
    rig.events()
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::Asked(ask) => match ask.kind {
                AskKind::Permission { subject, .. } => Some(subject),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn an_edit_outside_the_project_asks_even_when_edits_are_accepted() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let elsewhere = tempfile::tempdir().expect("dir");
    let target = elsewhere.path().join(".zshrc");
    let asked = run_answering(&rig, &writes(&[&target]), vec![Decision::Deny]);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].contains(".zshrc") && asked[0].contains("outside the project"), "{asked:?}");
    assert!(!target.exists(), "a denied write never happens");
}

#[test]
fn an_edit_outside_the_project_happens_once_the_user_allows_it() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let elsewhere = tempfile::tempdir().expect("dir");
    let target = elsewhere.path().join("notes.md");
    let asked = run_answering(&rig, &writes(&[&target]), vec![Decision::Allow]);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(target.exists());
}

#[test]
fn an_edit_in_an_added_folder_does_not_ask() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    let shared = tempfile::tempdir().expect("dir");
    rig.workspace.added_dirs = vec![shared.path().to_path_buf()];
    let target = shared.path().join("lib.rs");
    let asked = run_answering(&rig, &writes(&[&target]), Vec::new());
    assert!(asked.is_empty(), "{asked:?}");
    assert!(target.exists());
}

#[test]
fn an_edit_to_a_git_hook_asks_even_when_edits_are_accepted() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let asked = run_answering(&rig, &writes(&[Path::new(".git/hooks/pre-commit")]), vec![Decision::Deny]);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].contains(".git/hooks/pre-commit"), "{asked:?}");
    assert!(!rig.dir.path().join(".git/hooks/pre-commit").exists());
}

#[test]
fn an_edit_to_the_project_s_own_settings_asks_even_when_edits_are_accepted() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let settings = Path::new(brand::DIR).join("settings.local.json");
    let hook = Path::new(brand::DIR).join("hooks/start.js");
    let asked = run_answering(&rig, &writes(&[&settings, &hook]), vec![Decision::Deny, Decision::Deny]);
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(!rig.dir.path().join(&settings).exists());
    assert!(!rig.dir.path().join(&hook).exists());
}

#[test]
fn an_ordinary_edit_inside_the_project_still_does_not_ask() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let asked = run_answering(&rig, &writes(&[Path::new("src/new.rs")]), Vec::new());
    assert!(asked.is_empty(), "{asked:?}");
    assert!(rig.dir.path().join("src/new.rs").exists());
}

#[test]
fn bypassing_writes_protected_and_outside_files_without_asking() {
    let rig = Rig::new(PermissionMode::Bypass);
    let elsewhere = tempfile::tempdir().expect("dir");
    let outside = elsewhere.path().join("a.txt");
    let asked = run_answering(&rig, &writes(&[&outside, Path::new(".git/hooks/x")]), Vec::new());
    assert!(asked.is_empty(), "{asked:?}");
    assert!(outside.exists() && rig.dir.path().join(".git/hooks/x").exists());
}

#[test]
fn planning_refuses_an_edit_outside_the_project_without_asking() {
    let rig = Rig::new(PermissionMode::Plan);
    let elsewhere = tempfile::tempdir().expect("dir");
    let outside = elsewhere.path().join("a.txt");
    let asked = run_answering(&rig, &writes(&[&outside]), Vec::new());
    assert!(asked.is_empty(), "{asked:?}");
    assert!(!outside.exists());
}

#[test]
fn always_allowing_edits_does_not_cover_a_protected_file() {
    let rig = Rig::new(PermissionMode::Ask);
    let model = writes(&[Path::new("a.txt"), Path::new("b.txt"), Path::new(".git/hooks/x")]);
    let asked = run_answering(&rig, &model, vec![Decision::AllowAlways, Decision::Deny]);
    assert_eq!(asked.len(), 2, "the first edit and the protected one: {asked:?}");
    assert!(rig.dir.path().join("b.txt").exists(), "always covers ordinary edits");
    assert!(!rig.dir.path().join(".git/hooks/x").exists());
}

#[test]
fn a_deny_rule_still_refuses_a_protected_edit_without_asking() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    rig.workspace.policy.permissions = PermissionRules::default().with(RuleKind::Deny, "Edit(.git/**)");
    let asked = run_answering(&rig, &writes(&[Path::new(".git/hooks/x")]), Vec::new());
    assert!(asked.is_empty(), "{asked:?}");
    assert!(!rig.dir.path().join(".git/hooks/x").exists());
}

#[test]
fn only_an_allow_rule_naming_the_path_lets_a_protected_edit_through() {
    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    rig.workspace.policy.permissions = PermissionRules::default().with(RuleKind::Allow, "Edit");
    let asked = run_answering(&rig, &writes(&[Path::new(".git/hooks/x")]), vec![Decision::Deny]);
    assert_eq!(asked.len(), 1, "a bare Edit rule does not reach it: {asked:?}");

    let mut rig = Rig::new(PermissionMode::AcceptEdits);
    rig.workspace = agent_engine::tools::Workspace::new(rig.dir.path().canonicalize().expect("real"));
    rig.workspace.policy.permissions = PermissionRules::default().with(RuleKind::Allow, "Edit(.git/hooks/**)");
    let asked = run_answering(&rig, &writes(&[Path::new(".git/hooks/x")]), Vec::new());
    assert!(asked.is_empty(), "{asked:?}");
    assert!(rig.dir.path().join(".git/hooks/x").exists());
}

#[cfg(unix)]
#[test]
fn an_edit_through_a_link_that_leaves_the_project_asks() {
    let rig = Rig::new(PermissionMode::AcceptEdits);
    let elsewhere = tempfile::tempdir().expect("dir");
    std::os::unix::fs::symlink(elsewhere.path(), rig.dir.path().join("link")).expect("link");
    let asked = run_answering(&rig, &writes(&[Path::new("link/a.txt")]), vec![Decision::Deny]);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(!elsewhere.path().join("a.txt").exists());
}
