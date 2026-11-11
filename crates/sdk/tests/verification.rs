//! A session's verification criteria: the user's, or the last plan's,
//! sent with every run that is not planning.

use std::sync::Arc;

use backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
use domain::{Criteria, PermissionMode, SessionId};
use agent_engine::fake::FakeEngine;
use sdk::{EngineEvent, Model, Superfik};

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn superfik(engine: &FakeEngine, data: &std::path::Path) -> Superfik {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Superfik::builder().data_dir(data).services(services).build().expect("superfik")
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
    let superfik = superfik(&engine, data.path());
    let mut session = superfik.session(data.path(), model()).permission(PermissionMode::Plan);
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
    let superfik = superfik(&engine, data.path());
    let mut session = superfik.session(data.path(), model());
    session.set_verification(Some("the app starts".into()));
    session.send("do it").expect("run").text().expect("text");
    session.set_verification(None);
    session.send("again").expect("run").text().expect("text");
    let requests = engine.requests();
    assert_eq!(requests[0].verification, Some(Criteria::user("the app starts")));
    assert_eq!(requests[1].verification, None, "a run outside planning leaves the plan's criteria as they were");
}
