use std::sync::Arc;
use std::time::{Duration, Instant};

use superfik_core::{EngineEvent, EngineKind, ModelOption, PromptRequest, RunId, SessionId};
use superfik_engine::EngineRegistry;
use superfik_engine::fake::FakeEngine;
use superfik_protocol::{Backend, ClientCommand, Envelope, ServerEvent};

use super::*;
use crate::{InMemorySecretStore, InMemoryStore, Memories};

fn services(engine: &FakeEngine) -> LocalServices {
    let mut services = LocalServices::with_engines(EngineRegistry::new().with(Arc::new(engine.clone())));
    services.memory = Memories::plain(Arc::new(InMemoryStore::default()));
    services.memory_tool = false;
    let secrets = Arc::new(InMemorySecretStore::default());
    services.plugins = services.plugins.with_secrets(secrets.clone());
    services.providers = services.providers.with_secrets(secrets);
    services
}

fn options() -> DaemonOptions {
    DaemonOptions { idle: Duration::from_secs(60), build: "test-build".into() }
}

fn client(dir: &DaemonDir) -> SocketBackend {
    SocketBackend::connect(dir, Arc::new(|| {})).expect("connect")
}

fn until(backend: &SocketBackend, mut wanted: impl FnMut(&ServerEvent) -> bool) -> Vec<ServerEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "timed out; saw {seen:?}");
        match backend.try_recv() {
            Some(event) => {
                let done = wanted(&event);
                seen.push(event);
                if done {
                    return seen;
                }
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

#[test]
fn clients_share_one_daemon_and_each_sees_only_its_own_runs() {
    let temp = tempfile::tempdir().expect("dir");
    let dir = DaemonDir(temp.path().join("d"));
    let model = ModelOption::new(EngineKind::Native, "m", "M");
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::TextDelta("hi".into()),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(EngineKind::Native, vec![model.clone()], script);
    let daemon = Daemon::start(&dir, services(&engine), options()).expect("start").expect("first daemon");
    assert!(Daemon::start(&dir, services(&engine), options()).expect("second").is_none(), "one per folder");
    assert_eq!(dir.info().map(|(_, build)| build), Some("test-build".into()));

    let (one, two) = (client(&dir), client(&dir));
    for backend in [&one, &two] {
        backend.send(Envelope::local(ClientCommand::ListModels)).expect("send");
    }
    for backend in [&one, &two] {
        until(backend, |event| matches!(event, ServerEvent::Models(models) if models.len() == 1));
    }

    let run = RunId::new();
    let request = PromptRequest::plain(temp.path().into(), "hello", model);
    one.send(Envelope::local(ClientCommand::SubmitPrompt { run, request })).expect("submit");
    let seen = until(&one, |event| matches!(event, ServerEvent::Run { event: EngineEvent::Completed { .. }, .. }));
    assert!(seen.iter().any(|e| matches!(e, ServerEvent::Run { run: r, .. } if *r == run)));
    std::thread::sleep(Duration::from_millis(100));
    let mut other = Vec::new();
    while let Some(event) = two.try_recv() {
        other.push(event);
    }
    assert!(!other.iter().any(|e| matches!(e, ServerEvent::Run { .. })), "the other client saw {other:?}");

    drop((one, two));
    daemon.stop();
    assert!(!dir.socket().exists(), "the socket goes with the daemon");
    assert!(SocketBackend::connect(&dir, Arc::new(|| {})).is_err());
}

#[test]
fn a_client_starts_the_daemon_when_none_answers_and_it_leaves_when_idle() {
    let temp = tempfile::tempdir().expect("dir");
    let dir = DaemonDir(temp.path().join("d"));
    let engine = FakeEngine::new(EngineKind::Native, Vec::new(), Vec::new());
    let started = std::sync::Mutex::new(None);
    let backend = connect_or_start(&dir, Arc::new(|| {}), || {
        let options = DaemonOptions { idle: Duration::from_millis(200), build: "b".into() };
        *started.lock().expect("lock") = Daemon::start(&dir, services(&engine), options)?;
        Ok(())
    })
    .expect("connected");
    backend.send(Envelope::local(ClientCommand::ListProviders)).expect("send");
    until(&backend, |event| matches!(event, ServerEvent::Providers(_)));
    drop(backend);
    let deadline = Instant::now() + Duration::from_secs(5);
    while dir.socket().exists() {
        assert!(Instant::now() < deadline, "the idle daemon never left");
        std::thread::sleep(Duration::from_millis(50));
    }
}
