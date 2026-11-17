#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{Client, EngineEvent, Model};
use app::{InMemorySecretStore, LocalServices};
use domain::{Criteria, PermissionMode, SessionId};

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn client(engine: &FakeEngine, data: &std::path::Path) -> Client {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Client::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
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
    let client = client(&engine, data.path());
    let mut session = client.session(data.path(), model()).permission(PermissionMode::Plan);
    session.send("plan it").expect("run").text().expect("text");
    assert_eq!(session.verification(), Some(Criteria::plan("cargo test passes")));
    session.set_permission(PermissionMode::AcceptEdits);
    session.send("do it").expect("run").text().expect("text");
    let requests = engine.requests();
    assert_eq!(requests[0].policy.verification, None, "the planning run itself is not judged");
    assert_eq!(requests[1].policy.verification, Some(Criteria::plan("cargo test passes")));
}

#[test]
fn the_users_criteria_win_and_can_be_cleared() {
    let data = tempfile::tempdir().expect("data");
    let engine = planner();
    let client = client(&engine, data.path());
    let mut session = client.session(data.path(), model());
    session.set_verification(Some("the app starts".into()));
    session.send("do it").expect("run").text().expect("text");
    session.set_verification(None);
    session.send("again").expect("run").text().expect("text");
    let requests = engine.requests();
    assert_eq!(requests[0].policy.verification, Some(Criteria::user("the app starts")));
    assert_eq!(requests[1].policy.verification, None, "a run outside planning leaves the plan's criteria as they were");
}
