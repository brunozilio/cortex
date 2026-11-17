#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{Client, EngineEvent, Model};
use app::{InMemorySecretStore, LocalServices};
use domain::SessionId;
use domain::billing::ModelUsage;
use domain::budget::Budget;

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn costing(cost: f64) -> FakeEngine {
    FakeEngine::new(
        vec![model()],
        vec![
            EngineEvent::SessionStarted(SessionId("s-1".into())),
            EngineEvent::Usage(ModelUsage { estimated_cost_usd: Some(cost), ..ModelUsage::default() }),
            EngineEvent::TextDelta("ok".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: Some(cost) },
        ],
    )
}

#[test]
fn each_run_carries_the_budget_and_what_the_session_has_spent() {
    let data = tempfile::tempdir().expect("data");
    let engine = costing(0.75);
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    let client = Client::builder().data_dir(data.path()).services(services).build().expect(brand::COMMAND);
    let session = client.session(data.path(), model());

    session.send("one").expect("run").text().expect("text");
    session.set_budget(Some(5.0));
    session.send("two").expect("run").text().expect("text");
    session.send("three").expect("run").text().expect("text");
    session.set_budget(None);
    session.send("four").expect("run").text().expect("text");

    let budgets: Vec<Option<Budget>> = engine.requests().into_iter().map(|request| request.policy.budget).collect();
    assert_eq!(
        budgets,
        [None, Some(Budget { limit_usd: 5.0, spent_usd: 0.75 }), Some(Budget { limit_usd: 5.0, spent_usd: 1.5 }), None]
    );
    assert!((session.spending().spent_usd - 3.0).abs() < 1e-9);
}
