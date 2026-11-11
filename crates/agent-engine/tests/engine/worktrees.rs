use std::path::Path;
use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::transcript::Block;
use domain::{EngineEvent, PermissionMode, PromptRequest, SessionId};
use serde_json::{Value, json};

use crate::{Source, finish, model, start};

fn repo(dir: &Path) {
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-b", "main"]);
    std::fs::write(dir.join("README.md"), "base").expect("readme");
    git(&["add", "README.md"]);
    git(&["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
}

fn script(steps: Vec<(&str, &str, Value)>, last: &str) -> Arc<ScriptedModel> {
    let mut replies: Vec<_> = steps
        .into_iter()
        .map(|(id, name, input)| ScriptedModel::reply(vec![Block::ToolUse { id: id.into(), name: name.into(), input }]))
        .collect();
    replies.push(ScriptedModel::reply(vec![Block::Text(last.into())]));
    Arc::new(ScriptedModel::new(replies))
}

fn result<'a>(events: &'a [EngineEvent], id: &str) -> (&'a str, bool) {
    events
        .iter()
        .find_map(|event| match event {
            EngineEvent::ToolResult { id: found, output, failed } if found == id => Some((output.as_str(), *failed)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no result for {id}: {events:?}"))
}

fn moved(events: &[EngineEvent]) -> Vec<std::path::PathBuf> {
    events
        .iter()
        .filter_map(|event| if let EngineEvent::Moved(path) = event { Some(path.clone()) } else { None })
        .collect()
}

#[test]
fn the_session_moves_into_a_worktree_and_back() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().canonicalize().expect("real");
    repo(&project);
    let runs = vec![
        script(
            vec![
                ("enter", "EnterWorktree", json!({})),
                ("write", "Write", json!({"file_path": "work.txt", "content": "In the worktree"})),
            ],
            "Entered.",
        ),
        script(
            vec![
                ("read", "Read", json!({"file_path": "work.txt"})),
                ("refused", "ExitWorktree", json!({"action": "remove"})),
                ("exit", "ExitWorktree", json!({"action": "remove", "discard_changes": true})),
                ("after", "Write", json!({"file_path": "after.txt", "content": "Back home"})),
            ],
            "Left.",
        ),
    ];
    let store = tempfile::tempdir().expect("store");
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(runs))),
        Arc::new(agent_engine::sessions::FileSessions::new(store.path())),
    );
    let mut request = PromptRequest::plain(project.clone(), "Work apart", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;

    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let [worktree] = moved(&events).try_into().unwrap_or_else(|moves| panic!("one move: {moves:?}"));
    assert!(worktree.starts_with(project.join(concat!(brand::dir!(), "/worktrees"))), "{}", worktree.display());
    assert!(!result(&events, "enter").1);
    assert_eq!(std::fs::read_to_string(worktree.join("work.txt")).expect("work"), "In the worktree");
    assert!(!project.join("work.txt").exists(), "the project stays as it was");

    let session: SessionId = events
        .iter()
        .find_map(|event| if let EngineEvent::SessionStarted(id) = event { Some(id.clone()) } else { None })
        .expect("session");
    request.resume = Some(session);
    request.prompt = "Go on".into();
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let (read, failed) = result(&events, "read");
    assert!(!failed && read.contains("In the worktree"), "the next run goes on in the worktree: {read}");
    let (refused, failed) = result(&events, "refused");
    assert!(failed && refused.contains("discard_changes"), "{refused}");
    assert!(!result(&events, "exit").1);
    assert_eq!(moved(&events), std::slice::from_ref(&project));
    assert!(!worktree.exists(), "removed");
    assert_eq!(std::fs::read_to_string(project.join("after.txt")).expect("after"), "Back home");
}

#[test]
fn only_the_main_agent_outside_a_worktree_enters_one() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().canonicalize().expect("real");
    repo(&project);
    let run = script(
        vec![
            ("leave", "ExitWorktree", json!({"action": "keep"})),
            ("enter", "EnterWorktree", json!({})),
            ("again", "EnterWorktree", json!({})),
        ],
        "Done.",
    );
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![run]))),
        Arc::new(agent_engine::sessions::FileSessions::new(dir.path().join("sessions"))),
    );
    let mut request = PromptRequest::plain(project, "Go", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;
    let events = finish(&start(&engine, &request));
    let (leave, failed) = result(&events, "leave");
    assert!(failed && leave.contains("not in a worktree"), "{leave}");
    assert!(!result(&events, "enter").1);
    let (again, failed) = result(&events, "again");
    assert!(failed && again.contains("already"), "{again}");
}

#[test]
fn a_worktree_entered_in_a_run_that_then_fails_is_still_the_session_s() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().canonicalize().expect("real");
    repo(&project);
    let run = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::ToolUse {
        id: "enter".into(),
        name: "EnterWorktree".into(),
        input: json!({}),
    }])]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![run]))),
        Arc::new(agent_engine::sessions::FileSessions::new(dir.path().join("sessions"))),
    );
    let mut request = PromptRequest::plain(project.clone(), "Go", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;
    let events = finish(&start(&engine, &request));
    let [worktree] = moved(&events).try_into().unwrap_or_else(|moves| panic!("one move: {moves:?}"));
    assert!(matches!(events.last(), Some(EngineEvent::Failed(_))), "{events:?}");
    assert!(worktree.exists(), "kept for the session, not leaked or lost");
}
