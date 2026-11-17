#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;

use agent_engine::fake::FakeEngine;
use app::{Client, EngineEvent, Model};
use app::{InMemorySecretStore, LocalServices};
use domain::{SessionCommand, SessionId};

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

#[test]
fn a_lesson_is_asked_for_only_after_the_user_corrected_the_agent_enough() {
    let data = tempfile::tempdir().expect("data");
    let engine = FakeEngine::new(
        vec![model()],
        vec![
            EngineEvent::SessionStarted(SessionId("s-1".into())),
            EngineEvent::Steered("no, use pnpm".into()),
            EngineEvent::Steered("again: pnpm, not npm".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: None },
        ],
    );
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    let client = Client::builder().data_dir(data.path()).services(services).build().expect(brand::COMMAND);
    let session = client.session(data.path(), model());
    assert!(session.learn().is_none(), "nothing happened yet");

    session.send("install it").expect("run").text().expect("text");
    session.learn().expect("worth a lesson").expect("started").text().expect("text");

    let learned = engine.requests().into_iter().find(|request| request.session.command == Some(SessionCommand::Learn));
    let prompt = learned.map(|request| request.prompt.text).unwrap_or_default();
    assert!(prompt.contains("no, use pnpm") && prompt.contains("again: pnpm, not npm"), "{prompt}");
    assert!(session.learn().is_none(), "the signals were used up");
}
