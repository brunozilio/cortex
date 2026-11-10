//! A run from start to its terminal event: prompts, failures, panics and
//! project rules delivered on the way.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::{EngineEvent, EngineProblem, PromptRequest};
use superfik_native::model::ModelApi;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::sessions::InMemorySessions;
use superfik_native::transcript::{Block, Role};
use superfik_native::{ApiSource, NativeEngine};

use crate::{Source, finish, model, start};

#[test]
fn runs_a_prompt_and_continues_the_session() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("CLAUDE.md"), "Answer in one word.").expect("write");
    let first = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "g".into(),
            name: "Glob".into(),
            input: json!({"pattern": "*.md"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("CLAUDE.md".into())]),
    ]));
    let second = Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("Yes.".into())])]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&first), Arc::clone(&second)]))),
        Arc::new(InMemorySessions::default()),
    );

    let request = PromptRequest::plain(dir.path().into(), "Which docs are here?", model());
    let events = finish(&start(&engine, &request));
    let Some(EngineEvent::SessionStarted(session)) = events.first().cloned() else { panic!("{events:?}") };
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    assert!(events.contains(&EngineEvent::TextDelta("CLAUDE.md".into())));
    let system = &first.asked()[0].system;
    assert!(system.contains("Answer in one word."), "the project's instructions reach the model");

    let mut follow = PromptRequest::plain(dir.path().into(), "Sure?", model());
    follow.resume = Some(session.clone());
    let events = finish(&start(&engine, &follow));
    assert_eq!(events.first(), Some(&EngineEvent::SessionStarted(session)));
    let history = &second.asked()[0].messages;
    assert_eq!(history.len(), 5, "the first prompt's four messages, then the new one");
    assert_eq!(history[4].role, Role::User);
    assert_eq!(history[4].text(), "Sure?");
}

#[test]
fn without_an_account_it_says_so_and_fails() {
    let dir = tempfile::tempdir().expect("dir");
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(Vec::new()))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "hi", model())));
    assert!(events.contains(&EngineEvent::Diagnosed(EngineProblem::NotLoggedIn)), "{events:?}");
    assert!(matches!(events.last(), Some(EngineEvent::Failed(_))));
}

#[test]
fn provider_failure_during_turn_remains_a_failure_and_is_not_relabelled_cancelled() {
    let dir = tempfile::tempdir().expect("dir");
    let failing = Arc::new(ScriptedModel::new([Err(superfik_native::model::ModelError::Provider(
        "provider fixture failure".into(),
    ))]));
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(vec![failing]))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "test", model())));
    assert!(
        matches!(events.last(),Some(EngineEvent::Failed(message)) if message=="provider fixture failure"),
        "{events:?}"
    );
    assert!(!events.contains(&EngineEvent::Cancelled));
}

#[test]
fn an_unexpected_provider_panic_still_emits_one_terminal_failure() {
    struct Panicking;
    impl ApiSource for Panicking {
        fn api(&self, _: &PromptRequest) -> Result<Arc<dyn ModelApi>, superfik_native::model::ModelError> {
            panic!("synthetic provider panic")
        }
    }
    let dir = tempfile::tempdir().expect("dir");
    let engine = NativeEngine::new(Arc::new(Panicking), Arc::new(InMemorySessions::default()));
    let events = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "test", model())));
    assert!(matches!(events.last(),Some(EngineEvent::Failed(message)) if message.contains("unexpectedly")));
    assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
}

#[test]
fn nested_instruction_is_delivered_before_the_first_matching_tool_batch_executes() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/CLAUDE.md"), "NESTED-INSTRUCTION: use checked arithmetic.").expect("nested");
    std::fs::write(dir.path().join("src/file.txt"), "content").expect("file");
    let call = || {
        ScriptedModel::reply(vec![Block::ToolUse {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Read".into(),
            input: json!({"file_path":"src/file.txt"}),
        }])
    };
    let model = Arc::new(ScriptedModel::new([call(), call(), ScriptedModel::reply(vec![Block::Text("done".into())])]));
    let engine =
        NativeEngine::new(Arc::new(Source(Mutex::new(vec![model.clone()]))), Arc::new(InMemorySessions::default()));
    let events = finish(&start(
        &engine,
        &PromptRequest::plain(dir.path().into(), "read nested", superfik_core::ModelOption::new("fixture", "Fixture")),
    ));
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
    let asked = model.asked();
    assert!(!asked[0].system.contains("NESTED-INSTRUCTION"));
    assert!(asked[1].system.contains("NESTED-INSTRUCTION"));
    assert!(
        matches!(&asked[1].messages.last().expect("rule delivery").blocks[0],Block::ToolResult{is_error:true,content,..} if content.contains("NESTED-INSTRUCTION"))
    );
    assert!(
        matches!(&asked[2].messages.last().expect("file").blocks[0],Block::ToolResult{is_error:false,content,..} if content.contains("content"))
    );
}
