//! Events that reach a client's inbox while its run works go to the model
//! with the next tool results.

use std::sync::{Arc, Mutex};

use serde_json::json;
use superfik_core::inbox::Notice;
use superfik_core::{EngineEvent, PermissionMode, PromptRequest};
use superfik_native::NativeEngine;
use superfik_native::model::scripted::ScriptedModel;
use superfik_native::sessions::InMemorySessions;
use superfik_native::transcript::Block;

use crate::{Source, finish, model, start};

#[test]
fn a_notice_reaches_the_model_with_the_next_tool_results() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("a.txt"), "hello").expect("file");
    let scripted = Arc::new(ScriptedModel::new([
        ScriptedModel::reply(vec![Block::ToolUse {
            id: "r".into(),
            name: "Read".into(),
            input: json!({"file_path": "a.txt"}),
        }]),
        ScriptedModel::reply(vec![Block::Text("Seen.".into())]),
    ]));
    let engine = NativeEngine::new(
        Arc::new(Source(Mutex::new(vec![Arc::clone(&scripted)]))),
        Arc::new(InMemorySessions::default()),
    );
    let mut request = PromptRequest::plain(dir.path().into(), "read it", model());
    request.permission = PermissionMode::Bypass;
    request.safety_check = false;
    request.credentials.client = 93_001;
    superfik_native::tools::inbox::push(93_001, Notice::new("monitor bash_9", "ERROR: build failed"));
    let events = finish(&start(&engine, &request));
    assert!(events.contains(&EngineEvent::Notified(vec![Notice::new("monitor bash_9", "ERROR: build failed")])));
    let second = format!("{:?}", scripted.asked()[1].messages);
    assert!(second.contains("monitor bash_9") && second.contains("ERROR: build failed"), "{second}");
    assert!(superfik_native::tools::inbox::take(93_001).is_empty(), "taken by the run");
}
