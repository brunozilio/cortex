//! Prompts a client's runs scheduled, through the protocol: listed, due
//! ones handed over once, deleted, and only ever the client's own.

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use agent_engine::tools::schedules;
use backend::{InMemorySecretStore, InMemoryStore, LocalServices, Memories};
use domain::SessionId;
use sdk::{EngineEvent, Model, Sdk, When};

fn build(engine: &FakeEngine, data: &std::path::Path) -> Sdk {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Sdk::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

#[test]
fn a_client_takes_its_due_prompts_once_and_deletes_the_others() {
    let data = tempfile::tempdir().expect("data");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s-1".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(vec![Model::new("scripted", "Scripted")], script);
    let sdk = build(&engine, data.path());
    sdk.session(data.path(), Model::new("scripted", "Scripted")).send("hi").expect("run").for_each(drop);
    // What a run of this client scheduled (as `CronCreate` would).
    let client = engine.requests()[0].credentials.client;
    let now = schedules::now();
    schedules::add(client, "check the build", When::At(now - 1), now).expect("due");
    let later = schedules::add(client, "tomorrow", When::At(now + 86_400), now).expect("later");

    let found = sdk.schedules(None, true).expect("schedules");
    assert_eq!(found.due.iter().map(|due| due.prompt.as_str()).collect::<Vec<_>>(), ["check the build"]);
    assert_eq!(found.scheduled, std::slice::from_ref(&later));
    assert!(sdk.schedules(None, true).expect("again").due.is_empty(), "handed over once");

    let other = build(&engine, data.path());
    assert!(other.schedules(None, true).expect("other").scheduled.is_empty(), "another client sees none");

    let deleted = sdk.schedules(Some(&later.id), false).expect("deleted");
    assert!(deleted.scheduled.is_empty());
}
