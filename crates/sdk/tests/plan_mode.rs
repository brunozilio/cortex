use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
use domain::{AgentAsk, AskId, AskKind, Criteria, PermissionMode, SessionId};
use sdk::{EngineEvent, Model, Sdk};

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn sdk(engine: &FakeEngine, data: &std::path::Path) -> Sdk {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Sdk::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

const PLAN: &str = "## Approach\nEdit it.\n\n## Verification\ncargo test passes";

fn run(middle: Vec<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = vec![EngineEvent::SessionStarted(SessionId("s-1".into()))];
    events.extend(middle);
    events.push(EngineEvent::TextDelta("Implemented.".into()));
    events.push(EngineEvent::Completed { duration_ms: 1, cost_usd: None });
    events
}

fn presented() -> EngineEvent {
    EngineEvent::Asked(AgentAsk { id: AskId("p".into()), kind: AskKind::Plan { plan: PLAN.into() } })
}

#[test]
fn an_approved_plan_leaves_the_session_in_the_mode_chosen_with_the_plans_criteria() {
    let data = tempfile::tempdir().expect("data");
    let engine =
        FakeEngine::new(vec![model()], run(vec![presented(), EngineEvent::ModeChanged(PermissionMode::AcceptEdits)]));
    let sdk = sdk(&engine, data.path());
    let session = sdk.session(data.path(), model()).permission(PermissionMode::Plan);
    let events: Vec<EngineEvent> = session.send("plan it").expect("run").collect();
    assert!(matches!(events.last(), Some(EngineEvent::Completed { .. })));

    assert_eq!(session.permission_mode(), PermissionMode::AcceptEdits);
    assert_eq!(session.verification(), Some(Criteria::plan("cargo test passes")));
    session.send("more").expect("run").text().expect("text");
    let requests = engine.requests();
    assert_eq!(requests[1].permission, PermissionMode::AcceptEdits);
    assert_eq!(requests[1].verification, Some(Criteria::plan("cargo test passes")));
}

#[test]
fn entering_plan_mode_keeps_the_session_planning() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], run(vec![EngineEvent::ModeChanged(PermissionMode::Plan)]));
    let sdk = sdk(&engine, data.path());
    let session = sdk.session(data.path(), model());
    session.send("big change").expect("run").text().expect("text");
    assert_eq!(session.permission_mode(), PermissionMode::Plan);
}

#[test]
fn a_runs_text_includes_the_plan_it_presented() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(vec![model()], run(vec![presented()]));
    let sdk = sdk(&engine, data.path());
    let session = sdk.session(data.path(), model()).permission(PermissionMode::Plan);
    let text = session.send("plan it").expect("run").text().expect("text");
    assert!(text.contains("## Approach") && text.ends_with("Implemented."), "{text}");
    assert_eq!(session.permission_mode(), PermissionMode::Plan, "an unanswered plan keeps planning");
}
