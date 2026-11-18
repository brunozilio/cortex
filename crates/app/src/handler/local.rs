use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use crate::api::{ClientCommand, Envelope, ServerEvent};
use crossbeam_channel::{Receiver, Sender};

use crate::api::{Backend, BackendError, Wake};
use crate::handler::CommandHandler;
use crate::handler::outbox::Outbox;
use crate::services::LocalServices;

type Command = Envelope<ClientCommand>;

pub struct LocalBackend {
    workers: Workers,
    events: Receiver<ServerEvent>,
}

impl std::fmt::Debug for LocalBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalBackend").field("pending_events", &self.events.len()).finish_non_exhaustive()
    }
}

impl LocalBackend {
    pub fn new(services: LocalServices, wake: Wake) -> Result<Self, BackendError> {
        let (event_tx, events) = crossbeam_channel::unbounded();
        Ok(Self { workers: Workers::start(services, Outbox::new(event_tx, wake))?, events })
    }
}

impl Backend for LocalBackend {
    fn send(&self, command: Command) -> Result<(), BackendError> {
        self.workers.send(command)
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }

    fn alive(&self) -> bool {
        self.workers.vitals.alive()
    }
}

pub struct Workers {
    control: Option<Sender<Command>>,
    queries: Option<Sender<Command>>,
    handler: Arc<CommandHandler>,
    workers: Vec<JoinHandle<()>>,
    vitals: Vitals,
}

impl std::fmt::Debug for Workers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workers").field("threads", &self.workers.len()).finish_non_exhaustive()
    }
}

impl Workers {
    pub fn start(services: LocalServices, outbox: Outbox) -> Result<Self, BackendError> {
        let (vitals, wake) = (Vitals::default(), outbox.wake());
        let handler = Arc::new(CommandHandler::new(services, outbox));
        let (control, control_rx) = crossbeam_channel::unbounded();
        let (queries, queries_rx) = crossbeam_channel::unbounded();
        let workers = vec![
            spawn_worker("backend-control", control_rx, Arc::clone(&handler), vitals.watched(Arc::clone(&wake)))?,
            spawn_worker("backend-queries", queries_rx, Arc::clone(&handler), vitals.watched(wake))?,
        ];
        Ok(Self { control: Some(control), queries: Some(queries), handler, workers, vitals })
    }

    pub fn send(&self, command: Command) -> Result<(), BackendError> {
        if !self.vitals.alive() {
            return Err(BackendError::Disconnected);
        }
        let queue = if is_query(&command.payload) { &self.queries } else { &self.control };
        queue.as_ref().ok_or(BackendError::Disconnected)?.send(command).map_err(|_| BackendError::Disconnected)
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        self.control.take();
        self.queries.take();
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                tracing::error!("backend worker panicked");
            }
        }
        self.handler.cancel_all();
    }
}

fn is_query(command: &ClientCommand) -> bool {
    matches!(
        command,
        ClientCommand::RegisterPeer { .. }
            | ClientCommand::ListSkills { .. }
            | ClientCommand::ListAgents { .. }
            | ClientCommand::ListMcpServers { .. }
            | ClientCommand::ListPermissions { .. }
            | ClientCommand::TrustState { .. }
            | ClientCommand::TakeNotices { .. }
            | ClientCommand::AnswerTeammate { .. }
            | ClientCommand::Feature(_)
            | ClientCommand::ApproveMcpServer { .. }
            | ClientCommand::AddMcpServer { .. }
            | ClientCommand::GetMcpServer { .. }
            | ClientCommand::RemoveMcpServer { .. }
            | ClientCommand::SignInMcpServer { .. }
            | ClientCommand::SignOutMcpServer { .. }
            | ClientCommand::CancelRequest { .. }
    )
}

#[derive(Debug, Clone, Default)]
pub struct Vitals {
    dead: Arc<AtomicBool>,
}

impl Vitals {
    pub fn alive(&self) -> bool {
        !self.dead.load(Ordering::SeqCst)
    }

    fn died(&self) {
        self.dead.store(true, Ordering::SeqCst);
    }

    fn watched(&self, wake: Wake) -> Watched {
        Watched { vitals: self.clone(), wake }
    }
}

struct Watched {
    vitals: Vitals,
    wake: Wake,
}

impl Drop for Watched {
    fn drop(&mut self) {
        if thread::panicking() {
            tracing::error!("a backend worker died; every waiting call fails now");
            self.vitals.died();
            (self.wake)();
        }
    }
}

fn spawn_worker(
    name: &str,
    commands: Receiver<Command>,
    handler: Arc<CommandHandler>,
    watched: Watched,
) -> Result<JoinHandle<()>, BackendError> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let _watched = watched;
            for command in commands {
                handler.handle(command);
            }
        })
        .map_err(BackendError::Worker)
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
