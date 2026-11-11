use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
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

#[test]
fn a_rewind_puts_files_back_and_cuts_the_conversation() {
    let dir = tempfile::tempdir().expect("dir");
    let (notes, more) = (dir.path().join("notes.txt"), dir.path().join("more.txt"));
    let models = vec![writes(&notes.display().to_string(), "v1"), writes(&more.display().to_string(), "x")];
    let sessions = Arc::new(InMemorySessions::default());
    let engine = NativeEngine::new(Arc::new(Source(Mutex::new(models))), sessions.clone());

    let first = finish(&start(&engine, &PromptRequest::plain(dir.path().into(), "write the notes", model())));
    let Some(EngineEvent::SessionStarted(session)) = first.first().cloned() else { panic!("{first:?}") };
    assert!(first.contains(&EngineEvent::Checkpoint(0)), "{first:?}");
    let mut second = PromptRequest::plain(dir.path().into(), "add more", model());
    second.resume = Some(session.clone());
    assert!(finish(&start(&engine, &second)).contains(&EngineEvent::Checkpoint(1)));
    assert!(notes.exists() && more.exists());

    let rewind = |index: u32| {
        let mut request = PromptRequest::plain(dir.path().into(), "", model());
        request.resume = Some(session.clone());
        request.command = Some(SessionCommand::Rewind(index));
        let events = finish(&start(&engine, &request));
        assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })), "{events:?}");
        events.into_iter().find_map(|e| if let EngineEvent::Rewound(r) = e { Some(r) } else { None })
    };
    let history = |session: &SessionId| sessions.load_for(&second, session).expect("kept").len();
    let before_second = 4;
    assert_eq!(
        rewind(1),
        Some(Rewound { checkpoint: 1, prompt: "add more".into(), restored: vec![more.clone()], conversation: true })
    );
    assert!(!more.exists() && notes.exists(), "only what the second prompt wrote is undone");
    assert_eq!(history(&session), before_second);

    assert_eq!(rewind(0).map(|r| r.prompt), Some("write the notes".into()));
    assert!(!notes.exists());
    assert_eq!(history(&session), 0);
}
