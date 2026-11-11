use std::sync::{Arc, Mutex};

use agent_engine::NativeEngine;
use agent_engine::model::scripted::ScriptedModel;
use agent_engine::sessions::InMemorySessions;
use agent_engine::transcript::Block;
use domain::{Criteria, EngineEvent, PermissionMode, PromptRequest};

use crate::{Source, finish, model, start};

fn said(text: &str) -> Result<agent_engine::model::Reply, agent_engine::model::ModelError> {
    ScriptedModel::reply(vec![Block::Text(text.into())])
}

fn engine(model: &Arc<ScriptedModel>) -> NativeEngine {
    NativeEngine::new(Arc::new(Source(Mutex::new(vec![Arc::clone(model)]))), Arc::new(InMemorySessions::default()))
}

#[test]
fn a_request_with_criteria_is_judged_before_it_completes() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([said("Done"), said(r#"{"passed":true,"checks":[]}"#)]));
    let request = PromptRequest {
        verification: Some(Criteria::user("it is done")),
        permission: PermissionMode::Bypass,
        ..PromptRequest::plain(dir.path().into(), "do it", model())
    };
    let events = finish(&start(&engine(&scripted), &request));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Verified(v) if v.passed)), "{events:?}");
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })));
    assert!(scripted.asked()[1].messages[0].text().contains("it is done"));
}

#[test]
fn a_planning_run_is_never_judged() {
    let dir = tempfile::tempdir().expect("dir");
    let scripted = Arc::new(ScriptedModel::new([said("The plan")]));
    let request = PromptRequest {
        verification: Some(Criteria::plan("cargo test")),
        permission: PermissionMode::Plan,
        ..PromptRequest::plain(dir.path().into(), "plan it", model())
    };
    let events = finish(&start(&engine(&scripted), &request));
    assert!(!events.iter().any(|e| matches!(e, EngineEvent::Verified(_))));
    assert_eq!(scripted.asked().len(), 1);
}
