use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::{Block, Message};
use domain::{EngineEvent, EngineProblem, PromptRequest};
use serde_json::json;

use crate::{Source, finish, model, start};

#[test]
fn a_lost_session_is_diagnosed() {
    let dir = tempfile::tempdir().expect("dir");
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(Vec::new()))), Arc::new(InMemorySessions::default()));
    let mut request = PromptRequest::plain(dir.path().into(), "hi", model());
    request.resume = Some(domain::SessionId("gone".into()));
    let events = finish(&start(&engine, &request));
    assert_eq!(events.first(), Some(&EngineEvent::Diagnosed(EngineProblem::SessionNotFound)));
}

#[test]
fn a_fork_goes_on_under_a_new_session_and_leaves_the_original() {
    let dir = tempfile::tempdir().expect("dir");
    let first = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("One.".into())])]));
    let forked = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Two.".into())])]));
    let original = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Three.".into())])]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&first), Arc::clone(&forked), Arc::clone(&original)]))),
        Arc::new(InMemorySessions::default()),
    );
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "one", model())));
    let Some(EngineEvent::SessionStarted(session)) = events.first().cloned() else { panic!("{events:?}") };

    let mut fork = PromptRequest::plain(dir.path().into(), "two", model());
    fork.resume = Some(session.clone());
    fork.fork = true;
    let events = finish(&start(&engine, &fork));
    let Some(EngineEvent::SessionStarted(new)) = events.first().cloned() else { panic!("{events:?}") };
    assert_ne!(new, session, "the fork has an id of its own");
    assert_eq!(forked.asked()[0].messages.len(), 3, "it went on from the first conversation");

    let mut again = PromptRequest::plain(dir.path().into(), "three", model());
    again.resume = Some(session);
    finish(&start(&engine, &again));
    assert_eq!(original.asked()[0].messages.len(), 3, "the original never saw the fork's turn");
}

#[test]
fn measures_and_compacts_a_session() {
    let dir = tempfile::tempdir().expect("dir");
    let talk = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Hello there.".into())])]));
    let summary = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("They said hi.".into())])]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&talk), Arc::clone(&summary)]))),
        Arc::new(InMemorySessions::default()),
    );
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "hi", model())));
    let Some(EngineEvent::SessionStarted(session)) = events.first().cloned() else { panic!("{events:?}") };

    let mut measure = PromptRequest::plain(dir.path().into(), "", model());
    measure.resume = Some(session.clone());
    measure.command = Some(domain::SessionCommand::MeasureContext);
    let events = finish(&start(&engine, &measure));
    let usage =
        events.iter().find_map(|e| if let EngineEvent::ContextMeasured(u) = e { Some(u.clone()) } else { None });
    assert!(usage.is_some_and(|u| u.used > 0 && u.window > u.used), "{events:?}");
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })));

    let mut compact = measure.clone();
    compact.command = Some(domain::SessionCommand::Compact);
    let events = finish(&start(&engine, &compact));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Compacted(_))), "{events:?}");
    assert!(!summary.asked()[0].tools.is_empty(), "summarising keeps the tools so their cache is read");
    assert!(
        summary.asked()[0].messages.last().is_some_and(|m| m.text().contains("Do not call any tool")),
        "the prompt still forbids calling them"
    );

    let fresh = PromptRequest {
        command: Some(domain::SessionCommand::MeasureContext),
        ..PromptRequest::plain(dir.path().into(), "", model())
    };
    assert!(matches!(finish(&start(&engine, &fresh)).last(), Some(EngineEvent::Failed(_))), "no session to measure");
}

#[test]
fn sessions_are_bound_to_owner_and_directory_and_owner_cannot_come_from_wire() {
    use agent_engine::sessions::Sessions;
    use domain::{OrgId, Principal, UserId};
    let store = InMemorySessions::default();
    let dir = tempfile::tempdir().expect("valid test fixture");
    let mut request = PromptRequest::plain(dir.path().into(), "hello", model());
    request.credentials.owner = Some(Principal::member(UserId::new(), OrgId::new()));
    let id = domain::SessionId("private-session".into());
    store.save_for(&request, &id, &[Message::user("private")]).expect("valid test fixture");
    assert!(store.load_for(&request, &id).is_some());
    let mut other = request.clone();
    other.credentials.owner = Some(Principal::member(UserId::new(), OrgId::new()));
    assert!(store.load_for(&other, &id).is_none());
    let mut other = request.clone();
    other.cwd = dir.path().join("another-project");
    assert!(store.load_for(&other, &id).is_none());
    let serialized = serde_json::to_value(&request).expect("valid test fixture");
    assert!(serialized.get("credentials").is_none());
    let restored: PromptRequest = serde_json::from_value(serialized).expect("valid test fixture");
    assert!(restored.credentials.owner.is_none());
}
#[test]
fn corrupt_saved_history_fails_without_diagnosing_it_as_a_missing_session() {
    use agent_engine::sessions::{FileSessions, Sessions};
    let dir = tempfile::tempdir().expect("valid test fixture");
    let store = FileSessions::new(dir.path());
    let request = PromptRequest::plain(dir.path().into(), "hello", model());
    let id = domain::SessionId("broken-id".into());
    store.save_for(&request, &id, &[]).expect("save");
    let saved = std::fs::read_dir(dir.path())
        .expect("dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|ext| ext == "jsonl"));
    std::fs::write(saved.expect("the saved conversation"), "{broken").expect("corrupt it");
    assert!(store.load_for_checked(&request, &id).is_err());
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(Vec::new()))), Arc::new(store));
    let events = finish(&start(&engine, &PromptRequest { resume: Some(id), ..request }));
    assert!(matches!(events.last(),Some(EngineEvent::Failed(message)) if message.contains("restore")));
    assert!(!events.contains(&EngineEvent::Diagnosed(EngineProblem::SessionNotFound)));
}

#[test]
fn a_main_worktree_and_its_uncommitted_files_survive_resume_after_engine_restart() {
    let dir = tempfile::tempdir().expect("dir");
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("git")
                .success()
        );
    };
    git(&["init", "-b", "main"]);
    let root = dir.path().join(brand::DIR);
    std::fs::create_dir_all(root.join("agents")).expect("agents");
    std::fs::write(root.join("settings.json"), r#"{"agent":"reviewer"}"#).expect("settings");
    std::fs::write(root.join("agents/reviewer.md"), "---\nname: reviewer\nisolation: worktree\n---\nReview.")
        .expect("agent");
    git(&["add", brand::DIR]);
    git(&["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
    let first = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "write".into(),
            name: "Write".into(),
            input: json!({"file_path":"work.txt","content":"Persistent work"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Created".into())]),
    ]));
    let store = dir.path().join("sessions");
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![first.clone()]))),
        Arc::new(agent_engine::sessions::FileSessions::new(&store)),
    );
    let mut request = PromptRequest::plain(dir.path().into(), "Create work", model());
    request.permission = domain::PermissionMode::Bypass;
    request.safety_check = false;
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let session = events
        .iter()
        .find_map(|event| if let EngineEvent::SessionStarted(id) = event { Some(id.clone()) } else { None })
        .expect("session");
    drop(engine);
    let path = std::fs::read_dir(root.join("worktrees")).expect("worktrees").next().expect("one").expect("path").path();
    assert_eq!(std::fs::read_to_string(path.join("work.txt")).expect("work"), "Persistent work");
    assert!(!dir.path().join("work.txt").exists());
    let second = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "read".into(),
            name: "Read".into(),
            input: json!({"file_path":"work.txt"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Resumed".into())]),
    ]));
    use agent_engine::sessions::Sessions;
    struct Store {
        inner: agent_engine::sessions::FileSessions,
        fail: Arc<std::sync::atomic::AtomicBool>,
    }
    impl Sessions for Store {
        fn load(&self, id: &domain::SessionId) -> Option<Vec<Message>> {
            self.inner.load(id)
        }
        fn save(&self, id: &domain::SessionId, history: &[Message]) -> Result<(), String> {
            if self.fail.load(std::sync::atomic::Ordering::Relaxed) {
                Err("fixture store is read-only".into())
            } else {
                self.inner.save(id, history)
            }
        }
        fn load_worktree(
            &self,
            r: &PromptRequest,
            id: &domain::SessionId,
        ) -> Result<Option<agent_engine::agent_runtime::WorktreeState>, String> {
            self.inner.load_worktree(r, id)
        }
        fn save_worktree(
            &self,
            r: &PromptRequest,
            id: &domain::SessionId,
            state: &agent_engine::agent_runtime::WorktreeState,
        ) -> Result<(), String> {
            if self.fail.load(std::sync::atomic::Ordering::Relaxed) {
                Err("fixture store is read-only".into())
            } else {
                self.inner.save_worktree(r, id, state)
            }
        }
    }
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![second.clone()]))),
        Arc::new(Store { inner: agent_engine::sessions::FileSessions::new(&store), fail: fail.clone() }),
    );
    request.resume = Some(session);
    let failed = finish(&start(&engine, &request));
    assert!(matches!(failed.last(), Some(EngineEvent::Failed { .. })), "{failed:?}");
    assert!(path.join("work.txt").exists(), "persistence failure must never remove the existing checkout");
    assert!(second.asked().is_empty());
    fail.store(false, std::sync::atomic::Ordering::Relaxed);
    let events = finish(&start(&engine, &request));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let worktree = path.file_name().and_then(|name| name.to_str()).expect("worktree name");
    assert!(second.asked()[0].system.contains(worktree));
    assert!(format!("{:?}", second.asked()[1].messages).contains("Persistent work"));
    assert_eq!(std::fs::read_dir(root.join("worktrees")).expect("worktrees").count(), 1);
}
