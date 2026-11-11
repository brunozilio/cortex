use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use backend::{InMemorySecretStore, LocalServices};
use domain::{Criteria, PermissionMode, SessionId};
use sdk::{EngineEvent, Model, Sdk};

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn sdk(engine: &FakeEngine, data: &std::path::Path) -> Sdk {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.data = None;
    services.off.insert("memory_search".into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Sdk::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

fn planner() -> FakeEngine {
    FakeEngine::new(
        vec![model()],
        vec![
            EngineEvent::SessionStarted(SessionId("s-1".into())),
            EngineEvent::TextDelta("- **Approach**: edit it\n- **Verification**: ".into()),
            EngineEvent::TextDelta("cargo test passes".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: None },
        ],
    )
}

#[test]
fn a_plans_verification_is_sent_with_the_runs_that_carry_it_out() {
    let data = tempfile::tempdir().expect("data");
    let engine = planner();
    let sdk = sdk(&engine, data.path());
    let mut session = sdk.session(data.path(), model()).permission(PermissionMode::Plan);
    session.send("plan it").expect("run").text().expect("text");
    assert_eq!(session.verification(), Some(Criteria::plan("cargo test passes")));
    session.set_permission(PermissionMode::AcceptEdits);
    session.send("do it").expect("run").text().expect("text");
    let requests = engine.requests();
    assert_eq!(requests[0].verification, None, "the planning run itself is not judged");
    assert_eq!(requests[1].verification, Some(Criteria::plan("cargo test passes")));
}

#[test]
fn the_users_criteria_win_and_can_be_cleared() {
    let data = tempfile::tempdir().expect("data");
    let engine = planner();
    let sdk = sdk(&engine, data.path());
    let mut session = sdk.session(data.path(), model());
    session.set_verification(Some("the app starts".into()));
    session.send("do it").expect("run").text().expect("text");
    session.set_verification(None);
    session.send("again").expect("run").text().expect("text");
    let requests = engine.requests();
    assert_eq!(requests[0].verification, Some(Criteria::user("the app starts")));
    assert_eq!(requests[1].verification, None, "a run outside planning leaves the plan's criteria as they were");
}
