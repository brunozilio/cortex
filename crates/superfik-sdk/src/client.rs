//! The SDK's line to a [`Backend`]: commands go out with the local owner's
//! [`Envelope`]; a dispatcher thread routes each run's events to that run
//! and everything else to the one call waiting for an answer.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use superfik_core::{EngineEvent, RunId};
use superfik_protocol::{Backend, ClientCommand, Envelope, ServerEvent};

use crate::Error;

const ANSWER_TIMEOUT: Duration = Duration::from_secs(120);

type Routes = Arc<Mutex<HashMap<RunId, Sender<EngineEvent>>>>;

pub(crate) struct Client {
    backend: Arc<Mutex<Box<dyn Backend>>>,
    routes: Routes,
    replies: Receiver<ServerEvent>,
    calls: Mutex<()>,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

impl Client {
    /// `woken` fires whenever the backend has events (its `Wake`).
    pub(crate) fn new(backend: Box<dyn Backend>, woken: Receiver<()>) -> Self {
        let backend = Arc::new(Mutex::new(backend));
        let routes: Routes = Arc::default();
        let (reply_tx, replies) = crossbeam_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let (b, r, s) = (Arc::clone(&backend), Arc::clone(&routes), Arc::clone(&stop));
        std::thread::spawn(move || dispatch(&b, &r, &reply_tx, &woken, &s));
        Self { backend, routes, replies, calls: Mutex::new(()), stop }
    }

    pub(crate) fn send(&self, command: ClientCommand) -> Result<(), Error> {
        let backend = self.backend.lock().unwrap_or_else(PoisonError::into_inner);
        backend.send(Envelope::local(command)).map_err(|error| Error::Backend(error.to_string()))
    }

    /// Sends `command` and waits for the first answer `pick` accepts; an
    /// error the backend sends meanwhile fails the call.
    pub(crate) fn call<T>(
        &self,
        command: ClientCommand,
        mut pick: impl FnMut(ServerEvent) -> Option<Result<T, String>>,
    ) -> Result<T, Error> {
        let _one_at_a_time = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
        while self.replies.try_recv().is_ok() {}
        self.send(command)?;
        loop {
            match self.replies.recv_timeout(ANSWER_TIMEOUT) {
                Ok(ServerEvent::Error { message }) => return Err(Error::Backend(message)),
                Ok(event) => {
                    if let Some(answer) = pick(event) {
                        return answer.map_err(Error::Backend);
                    }
                }
                Err(_) => return Err(Error::Timeout),
            }
        }
    }

    /// Starts receiving a run's events (before its command is sent).
    pub(crate) fn follow(&self, run: RunId) -> Receiver<EngineEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.routes.lock().unwrap_or_else(PoisonError::into_inner).insert(run, tx);
        rx
    }

    pub(crate) fn forget(&self, run: RunId) {
        self.routes.lock().unwrap_or_else(PoisonError::into_inner).remove(&run);
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn dispatch(
    backend: &Mutex<Box<dyn Backend>>,
    routes: &Mutex<HashMap<RunId, Sender<EngineEvent>>>,
    replies: &Sender<ServerEvent>,
    woken: &Receiver<()>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        let _ = woken.recv_timeout(Duration::from_millis(250));
        loop {
            let event = backend.lock().unwrap_or_else(PoisonError::into_inner).try_recv();
            let Some(event) = event else { break };
            match event {
                ServerEvent::Run { run, event } => {
                    let mut routes = routes.lock().unwrap_or_else(PoisonError::into_inner);
                    let terminal = event.is_terminal();
                    if let Some(route) = routes.get(&run) {
                        let _ = route.send(event);
                    }
                    if terminal {
                        routes.remove(&run);
                    }
                }
                other => {
                    let _ = replies.send(other);
                }
            }
        }
    }
}
