use std::sync::{Arc, Mutex};

use agent_engine::Engine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::{InMemorySessions, Sessions};
use agent_engine::transcript::Block;
use domain::{EngineEvent, PromptRequest, Rewound, SessionCommand, SessionId};
use serde_json::json;

use crate::{Source, finish, model, start};

fn writes(file: &str, content: &str) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "w".into(),
            name: "Write".into(),
            input: json!({"file_path": file, "content": content}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Written.".into())]),
    ]))
}

#[cfg(unix)]
#[test]
fn a_rewind_also_undoes_what_a_shell_command_changed_in_the_repository() {
    let dir = tempfile::tempdir().expect("dir");
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git").args(args).current_dir(dir.path()).output().expect("git");
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    };
    git(&["init", "-q"]);
    std::fs::write(dir.path().join("a.txt"), "one\n").expect("write");
    std::fs::write(dir.path().join(".gitignore"), "build/\n").expect("ignore");
    git(&["add", "."]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "start"]);
    let shell = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "b".into(),
            name: "Bash".into(),
            input: json!({"command": "printf 'two\\n' > a.txt && printf new > b.txt && mkdir -p build && printf x > build/out"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Done.".into())]),
    ]));
    let sessions = Arc::new(InMemorySessions::default());
    let engine = Engine::new(Arc::new(Source(Mutex::new(vec![shell]))), sessions);
    let mut request = PromptRequest::plain(dir.path().into(), "change things", model());
    request.policy.allowed_tools = vec!["Bash".into()];
    let events = finish(&start(&engine, &request));
    let Some(EngineEvent::SessionStarted(session)) = events.first().cloned() else { panic!("{events:?}") };
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "two\n");

    let mut rewind = PromptRequest::plain(dir.path().into(), "", model());
    rewind.session.resume = Some(session);
    rewind.session.command = Some(SessionCommand::Rewind(0));
    let events = finish(&start(&engine, &rewind));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).expect("a"), "one\n", "the tracked file is back");
    assert!(!dir.path().join("b.txt").exists(), "the new file is gone");
    assert!(dir.path().join("build/out").exists(), "ignored build output is left alone");
}

#[test]
fn a_rewind_puts_files_back_and_cuts_the_conversation() {
    let dir = tempfile::tempdir().expect("dir");
    let (notes, more) = (dir.path().join("notes.txt"), dir.path().join("more.txt"));
    let models = vec![writes(&notes.display().to_string(), "v1"), writes(&more.display().to_string(), "x")];
    let sessions = Arc::new(InMemorySessions::default());
    let engine = Engine::new(Arc::new(Source(Mutex::new(models))), sessions.clone());

    let first = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "write the notes", model())));
    let Some(EngineEvent::SessionStarted(session)) = first.first().cloned() else { panic!("{first:?}") };
    assert!(first.contains(&EngineEvent::Checkpoint(0)), "{first:?}");
    let mut second = PromptRequest::plain(dir.path().into(), "add more", model());
    second.session.resume = Some(session.clone());
    assert!(finish(&start(&engine, &second)).contains(&EngineEvent::Checkpoint(1)));
    assert!(notes.exists() && more.exists());

    let rewind = |index: u32| {
        let mut request = PromptRequest::plain(dir.path().into(), "", model());
        request.session.resume = Some(session.clone());
        request.session.command = Some(SessionCommand::Rewind(index));
        let events = finish(&start(&engine, &request));
        assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
        events.into_iter().find_map(|e| if let EngineEvent::Rewound(r) = e { Some(r) } else { None })
    };
    let history = |session: &SessionId| sessions.load_for(&second.session.cwd, session).expect("kept").len();
    let before_second = 4;
    assert_eq!(
        rewind(1),
        Some(Rewound {
            checkpoint: 1,
            prompt: "add more".into(),
            restored: vec![more.clone()],
            conversation: true,
            skipped: Vec::new()
        })
    );
    assert!(!more.exists() && notes.exists(), "only what the second prompt wrote is undone");
    assert_eq!(history(&session), before_second);

    assert_eq!(rewind(0).map(|r| r.prompt), Some("write the notes".into()));
    assert!(!notes.exists());
    assert_eq!(history(&session), 0);
}

#[test]
fn a_branch_starts_a_new_conversation_before_a_prompt_and_keeps_the_original_and_the_files() {
    let dir = tempfile::tempdir().expect("dir");
    let (notes, more) = (dir.path().join("notes.txt"), dir.path().join("more.txt"));
    let models = vec![writes(&notes.display().to_string(), "v1"), writes(&more.display().to_string(), "x")];
    let sessions = Arc::new(InMemorySessions::default());
    let engine = Engine::new(Arc::new(Source(Mutex::new(models))), sessions.clone());
    let first = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "write the notes", model())));
    let Some(EngineEvent::SessionStarted(session)) = first.first().cloned() else { panic!("{first:?}") };
    let mut second = PromptRequest::plain(dir.path().into(), "add more", model());
    second.session.resume = Some(session.clone());
    finish(&start(&engine, &second));
    let request = PromptRequest::plain(dir.path().into(), "", model());
    let original = sessions.load_for(&request.session.cwd, &session).map(|history| history.len());

    let mut branch = PromptRequest::plain(dir.path().into(), "", model());
    branch.session.resume = Some(session.clone());
    branch.session.command = Some(SessionCommand::Branch(1));
    let events = finish(&start(&engine, &branch));

    let Some(EngineEvent::SessionStarted(branched)) = events.first().cloned() else { panic!("{events:?}") };
    assert_ne!(branched, session);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, EngineEvent::Branched { checkpoint: 1, prompt, .. } if prompt == "add more"))
    );
    let kept = sessions.load_for(&request.session.cwd, &branched).map(|history| history.len());
    assert!(kept.is_some_and(|kept| original.is_some_and(|original| kept < original)), "{kept:?} {original:?}");
    assert_eq!(
        sessions.load_for(&request.session.cwd, &session).map(|history| history.len()),
        original,
        "the original is kept"
    );
    assert!(notes.exists() && more.exists(), "a branch leaves the files as they are");
}
