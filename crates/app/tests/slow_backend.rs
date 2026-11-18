#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use agent_engine::fake::FakeEngine;
use app::{Client, EngineEvent, LocalServices, Model, SettingsStore};
use crossbeam_channel::{Receiver, Sender};
use domain::SessionId;

struct Stuck {
    entered: AtomicBool,
    released: Receiver<()>,
}

impl SettingsStore for Stuck {
    fn load(&self) -> Option<serde_json::Value> {
        self.entered.store(true, Ordering::SeqCst);
        let _ = self.released.recv();
        None
    }

    fn save(&self, _settings: &serde_json::Value) -> Result<(), app::SettingsError> {
        Ok(())
    }
}

fn model() -> Model {
    Model::new("scripted", "Scripted")
}

fn with_a_stuck_backend() -> (Client, Arc<Stuck>, Sender<()>, FakeEngine) {
    let script = vec![
        EngineEvent::SessionStarted(SessionId("s".into())),
        EngineEvent::Completed { duration_ms: 1, cost_usd: None },
    ];
    let engine = FakeEngine::new(vec![model()], script);
    let (release, released) = crossbeam_channel::unbounded();
    let stuck = Arc::new(Stuck { entered: AtomicBool::new(false), released });
    let mut services = LocalServices::with_engine(Arc::new(engine.clone()));
    services.off.insert(domain::features::MEMORY_SEARCH.into());
    services.settings = Arc::clone(&stuck) as Arc<dyn SettingsStore>;
    (Client::builder().services(services).build().expect(brand::COMMAND), stuck, release, engine)
}

#[test]
fn starting_a_run_a_side_question_or_a_context_measure_never_waits_for_the_backend() {
    let (client, stuck, release, engine) = with_a_stuck_backend();
    let asking = client.clone();
    let settings = std::thread::spawn(move || asking.settings());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !stuck.entered.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "the backend never took the command it is stuck on");
        std::thread::sleep(Duration::from_millis(2));
    }
    let session = client.session("/repo", model()).resume(SessionId("s".into()));

    let asked = Instant::now();
    let prompt = session.send_with_images("hi", Vec::new()).expect("enqueued");
    let side = session.side("what is this?").expect("enqueued");
    let measure = session.measure_context().expect("enqueued");
    let took = asked.elapsed();

    assert!(engine.requests().is_empty(), "the backend is still stuck on the command before them");
    assert!(took < Duration::from_secs(1), "each call only enqueues a command and registers a route: {took:?}");

    release.send(()).expect("released");
    assert!(settings.join().expect("join").is_ok());
    for run in [prompt, side, measure] {
        let ended = run.last();
        assert!(matches!(ended, Some(EngineEvent::Completed { .. })), "{ended:?}");
    }
    assert_eq!(engine.requests().len(), 3, "and all three ran once the backend was free");
}
