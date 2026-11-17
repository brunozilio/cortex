use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::api::{Backend, ClientCommand, Envelope, Reply, RequestId, ServerEvent, Unsolicited};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use domain::{EngineEvent, RunId};
use platform::sync::Locked;

use crate::Error;

const ANSWER_TIMEOUT: Duration = Duration::from_secs(120);
const SIGN_IN_MARGIN: Duration = Duration::from_secs(30);
const UNSOLICITED_KEPT: usize = 256;
const SHUT_DOWN: &str = "the backend has shut down";

pub(crate) fn sign_in_patience() -> Duration {
    crate::api::SIGN_IN_TIMEOUT + SIGN_IN_MARGIN
}

struct Waiter {
    replies: Sender<Reply>,
    long: bool,
}

#[derive(Default)]
struct Shared {
    routes: Mutex<HashMap<RunId, Sender<EngineEvent>>>,
    waiters: Mutex<HashMap<RequestId, Waiter>>,
    unsolicited: Mutex<VecDeque<Unsolicited>>,
    closed: AtomicBool,
}

impl Shared {
    fn route(&self, event: ServerEvent) {
        match event {
            ServerEvent::Run { run, event } => {
                let mut routes = self.routes.locked();
                let terminal = event.is_terminal();
                if let Some(route) = routes.get(&run) {
                    let _ = route.send(event);
                }
                if terminal {
                    routes.remove(&run);
                }
            }
            ServerEvent::Reply { id, reply } => self.answer(id, reply),
            ServerEvent::Unsolicited(unsolicited) => {
                let mut kept = self.unsolicited.locked();
                if kept.len() == UNSOLICITED_KEPT {
                    kept.pop_front();
                }
                kept.push_back(unsolicited);
            }
        }
    }

    fn answer(&self, id: RequestId, reply: Reply) {
        let mut waiters = self.waiters.locked();
        let terminal = reply.is_terminal();
        match waiters.get(&id) {
            Some(waiter) => {
                let _ = waiter.replies.send(reply);
                if terminal {
                    waiters.remove(&id);
                }
            }
            None => match reply {
                Reply::Error { message } => tracing::warn!(request = %id, %message, "a command nobody waits on failed"),
                _ => tracing::debug!(request = %id, "dropping a reply nobody waits for"),
            },
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.waiters.locked().clear();
        self.routes.locked().clear();
    }
}

pub(crate) struct Pending {
    id: RequestId,
    replies: Receiver<Reply>,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending").field("id", &self.id).finish_non_exhaustive()
    }
}

impl Pending {
    pub(crate) fn replies(&self) -> &Receiver<Reply> {
        &self.replies
    }

    fn answer<T>(
        &self,
        within: Duration,
        mut pick: impl FnMut(Reply) -> Option<Result<T, String>>,
    ) -> Result<T, Error> {
        let deadline = std::time::Instant::now() + within;
        loop {
            match self.replies.recv_deadline(deadline) {
                Ok(Reply::Error { message }) => return Err(Error::Backend(message)),
                Ok(reply) => {
                    let terminal = reply.is_terminal();
                    if let Some(answer) = pick(reply) {
                        return answer.map_err(Error::Backend);
                    }
                    if terminal {
                        return Err(Error::Backend("the backend answered something else".into()));
                    }
                }
                Err(RecvTimeoutError::Timeout) => return Err(Error::Timeout),
                Err(RecvTimeoutError::Disconnected) => return Err(Error::Backend(SHUT_DOWN.into())),
            }
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.shared.waiters.locked().remove(&self.id);
    }
}

pub(crate) struct Dispatcher {
    backend: Arc<Mutex<Box<dyn Backend>>>,
    shared: Arc<Shared>,
    next: AtomicU64,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for Dispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dispatcher").finish_non_exhaustive()
    }
}

impl Dispatcher {
    pub(crate) fn new(backend: Box<dyn Backend>, woken: Receiver<()>) -> Self {
        let backend = Arc::new(Mutex::new(backend));
        let shared = Arc::new(Shared::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (b, s, halt) = (Arc::clone(&backend), Arc::clone(&shared), Arc::clone(&stop));
        platform::named::spawn("client-dispatch", move || dispatch(&b, &s, &woken, &halt));
        Self { backend, shared, next: AtomicU64::new(1), stop }
    }

    fn post(&self, id: RequestId, command: ClientCommand) -> Result<(), Error> {
        let backend = self.backend.locked();
        backend.send(Envelope::local(id, command)).map_err(|error| Error::Backend(error.to_string()))
    }

    fn next_id(&self) -> RequestId {
        RequestId(self.next.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn send(&self, command: ClientCommand) -> Result<(), Error> {
        self.post(self.next_id(), command)
    }

    pub(crate) fn ask(&self, command: ClientCommand) -> Result<Pending, Error> {
        self.asking(command, false)
    }

    fn asking(&self, command: ClientCommand, long: bool) -> Result<Pending, Error> {
        let id = self.next_id();
        let (replies, answered) = crossbeam_channel::unbounded();
        self.shared.waiters.locked().insert(id, Waiter { replies, long });
        let pending = Pending { id, replies: answered, shared: Arc::clone(&self.shared) };
        if self.shared.closed.load(Ordering::SeqCst) {
            return Err(Error::Backend(SHUT_DOWN.into()));
        }
        self.post(id, command)?;
        Ok(pending)
    }

    pub(crate) fn call<T>(
        &self,
        command: ClientCommand,
        pick: impl FnMut(Reply) -> Option<Result<T, String>>,
    ) -> Result<T, Error> {
        self.asking(command, false)?.answer(ANSWER_TIMEOUT, pick)
    }

    pub(crate) fn flow<T>(
        &self,
        command: ClientCommand,
        within: Duration,
        pick: impl FnMut(Reply) -> Option<Result<T, String>>,
    ) -> Result<T, Error> {
        let pending = self.asking(command, true)?;
        let answer = pending.answer(within, pick);
        if matches!(answer, Err(Error::Timeout)) {
            self.cancel(pending.id);
        }
        answer
    }

    pub(crate) fn cancel_flows(&self) {
        let flows: Vec<RequestId> =
            self.shared.waiters.locked().iter().filter(|(_, waiter)| waiter.long).map(|(id, _)| *id).collect();
        for flow in flows {
            self.cancel(flow);
        }
    }

    fn cancel(&self, request: RequestId) {
        if let Err(error) = self.send(ClientCommand::CancelRequest { request }) {
            tracing::debug!(%request, %error, "could not cancel a request");
        }
    }

    pub(crate) fn unsolicited(&self) -> Vec<Unsolicited> {
        self.shared.unsolicited.locked().drain(..).collect()
    }

    pub(crate) fn follow(&self, run: RunId) -> Receiver<EngineEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.shared.routes.locked().insert(run, tx);
        if self.shared.closed.load(Ordering::SeqCst) {
            self.forget(run);
        }
        rx
    }

    pub(crate) fn forget(&self, run: RunId) {
        self.shared.routes.locked().remove(&run);
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn dispatch(backend: &Mutex<Box<dyn Backend>>, shared: &Shared, woken: &Receiver<()>, stop: &AtomicBool) {
    let mut reachable = true;
    while reachable && !stop.load(Ordering::Relaxed) {
        reachable = woken.recv_timeout(Duration::from_millis(250)) != Err(RecvTimeoutError::Disconnected);
        loop {
            let event = backend.locked().try_recv();
            let Some(event) = event else { break };
            shared.route(event);
        }
        reachable = reachable && backend.locked().alive();
    }
    shared.close();
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
