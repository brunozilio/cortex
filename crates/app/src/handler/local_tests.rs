use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::api::{Reply, RequestId};

use super::*;

const PROMPTLY: Duration = Duration::from_secs(5);

#[derive(Default)]
struct BreaksWhenArmed {
    armed: AtomicBool,
}

impl crate::Homes for BreaksWhenArmed {
    fn home(&self) -> Option<std::path::PathBuf> {
        assert!(!self.armed.load(Ordering::SeqCst), "the home folder blew up");
        None
    }
}

fn backend(homes: &Arc<BreaksWhenArmed>) -> LocalBackend {
    let mut services = crate::features::context::testing::services(None);
    services.homes = Arc::clone(homes) as Arc<dyn crate::Homes>;
    LocalBackend::new(services, Arc::new(|| {})).expect("backend")
}

fn answered(backend: &LocalBackend, id: u64, command: ClientCommand) -> Reply {
    let asked = Instant::now();
    backend.send(Envelope::local(RequestId(id), command)).expect("sent");
    loop {
        match backend.try_recv() {
            Some(ServerEvent::Reply { id: answered, reply }) if answered == RequestId(id) => return reply,
            Some(_) => {}
            None => {
                assert!(asked.elapsed() < PROMPTLY, "request {id} was not answered within {PROMPTLY:?}");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

#[test]
fn queries_are_routed_apart_from_run_control() {
    assert!(is_query(&ClientCommand::ListSkills { path: "/".into() }));
    assert!(is_query(&ClientCommand::ListAgents { path: "/".into() }));
    assert!(!is_query(&ClientCommand::ListModels));
    assert!(is_query(&ClientCommand::TrustState { path: "/".into() }), "reading the state waits behind no run");
    assert!(!is_query(&ClientCommand::Trust { path: "/".into(), trusted: true }), "a change is ordered with the runs");
    assert!(!is_query(&ClientCommand::TrustForThisProcess { path: "/".into() }));
}

#[test]
fn a_command_whose_handler_panics_is_answered_with_an_error_and_the_worker_keeps_serving() {
    let homes = Arc::new(BreaksWhenArmed::default());
    let backend = backend(&homes);
    let skills = || ClientCommand::ListSkills { path: "/".into() };

    homes.armed.store(true, Ordering::SeqCst);
    let control = answered(&backend, 1, ClientCommand::LoadSettings);
    let query = answered(&backend, 2, skills());
    assert_eq!(control, Reply::error("internal error while handling load_settings"));
    assert_eq!(query, Reply::error("internal error while handling list_skills"));

    homes.armed.store(false, Ordering::SeqCst);
    assert_eq!(answered(&backend, 3, ClientCommand::LoadSettings), Reply::Settings { settings: None });
    assert!(matches!(answered(&backend, 4, skills()), Reply::Skills(_)), "the queries worker serves on too");
    assert!(backend.alive());
}

#[test]
fn a_worker_that_dies_is_seen_at_once_and_one_that_was_told_to_stop_is_not_a_death() {
    let woken = Arc::new(AtomicUsize::new(0));
    let (counter, vitals) = (Arc::clone(&woken), Vitals::default());
    let wake: Wake = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });

    let stopped = vitals.watched(Arc::clone(&wake));
    drop(stopped);
    assert!(vitals.alive(), "a worker whose queue closed just ended");
    assert_eq!(woken.load(Ordering::SeqCst), 0);

    let dying = vitals.watched(wake);
    let died = std::thread::spawn(move || {
        let _held = dying;
        panic!("the worker blew up outside a command");
    })
    .join();
    assert!(died.is_err());
    assert!(!vitals.alive());
    assert_eq!(woken.load(Ordering::SeqCst), 1, "whoever waits on the backend is woken to see it");
}

#[test]
fn a_backend_whose_worker_died_refuses_what_is_sent_after() {
    let homes = Arc::new(BreaksWhenArmed::default());
    let backend = backend(&homes);
    backend.workers.vitals.died();
    assert!(!backend.alive());
    let refused = backend.send(Envelope::local(RequestId(1), ClientCommand::LoadSettings));
    assert!(matches!(refused, Err(BackendError::Disconnected)), "{refused:?}");
}
