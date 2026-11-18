#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;
use std::time::Duration;

use agent_engine::fake::FakeEngine;
use app::{Client, EngineEvent, Model};
use app::{InMemorySecretStore, LocalServices};
use domain::SessionId;
use domain::conversation::SessionStatus;

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn engine() -> FakeEngine {
    FakeEngine::new(
        vec![model()],
        vec![
            EngineEvent::SessionStarted(SessionId("s-1".into())),
            EngineEvent::TextDelta("Hello ".into()),
            EngineEvent::TextDelta("there".into()),
            EngineEvent::Completed { duration_ms: 1, cost_usd: None },
        ],
    )
}

fn build(engine: &FakeEngine, data: &std::path::Path) -> Client {
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    services.providers = services.providers.with_secrets(Arc::new(InMemorySecretStore::default()));
    Client::builder().data_dir(data).services(services).build().expect(brand::COMMAND)
}

#[test]
fn sessions_on_sibling_clients_run_at_the_same_time_without_mixing() {
    let data = tempfile::tempdir().expect("data");
    let first = build(&engine().with_delay(Duration::from_millis(20)), data.path());
    let second = first.sibling().expect("sibling");

    let runs: Vec<_> = [first.clone(), second]
        .into_iter()
        .map(|client| {
            let cwd = data.path().to_path_buf();
            std::thread::spawn(move || {
                let session = client.session(cwd, model());
                let run = session.send("hi").expect("run");
                let models = client.models().expect("a query while the run streams").len();
                (run.text().expect("text"), models)
            })
        })
        .collect();

    for run in runs {
        assert_eq!(run.join().expect("thread"), ("Hello there".to_owned(), 1));
    }
}

#[test]
fn every_client_sees_the_sessions_the_others_ran_and_can_tidy_them() {
    let data = tempfile::tempdir().expect("data");
    let first = build(&engine(), data.path());
    let second = first.sibling().expect("sibling");
    assert!(second.sessions().expect("sessions").is_empty());

    first.session(data.path(), model()).send("hi").expect("run").text().expect("text");

    let id = SessionId("s-1".into());
    let listed = second.sessions().expect("sessions");
    assert_eq!(listed.len(), 1);
    assert_eq!((&listed[0].session, listed[0].status, listed[0].unread), (&id, SessionStatus::Done, true));

    second.rename_session(&id, "Greeting").expect("rename");
    second.seen_session(&id).expect("seen");
    let row = first.sessions().expect("sessions").remove(0);
    assert_eq!((row.title.as_deref(), row.unread), (Some("Greeting"), false));

    assert!(first.history(&id, data.path()).expect("history").is_empty(), "the scripted engine keeps no log");
    assert_eq!(first.session_length(&id, data.path()).expect("length"), 0, "nor any length");
    first.delete_session(&id).expect("delete");
    assert!(second.sessions().expect("sessions").is_empty());
}
