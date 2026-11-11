use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use protocol::{ClientCommand, Envelope, ServerEvent};

use crate::handler::CommandHandler;
use crate::outbox::Outbox;
use crate::services::LocalServices;
use protocol::{Backend, BackendError, Wake};

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

    pub fn detect(wake: Wake) -> Result<Self, BackendError> {
        Self::new(LocalServices::detect(), wake)
    }
}

impl Backend for LocalBackend {
    fn send(&self, command: Command) -> Result<(), BackendError> {
        self.workers.send(command)
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }
}

pub struct Workers {
    control: Option<Sender<Command>>,
    queries: Option<Sender<Command>>,
    handler: Arc<CommandHandler>,
    workers: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for Workers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workers").field("threads", &self.workers.len()).finish_non_exhaustive()
    }
}

impl Workers {
    pub fn start(services: LocalServices, outbox: Outbox) -> Result<Self, BackendError> {
        let handler = Arc::new(CommandHandler::new(services, outbox));
        let (control, control_rx) = crossbeam_channel::unbounded();
        let (queries, queries_rx) = crossbeam_channel::unbounded();
        let workers = vec![
            spawn_worker("backend-control", control_rx, Arc::clone(&handler))?,
            spawn_worker("backend-queries", queries_rx, Arc::clone(&handler))?,
        ];
        Ok(Self { control: Some(control), queries: Some(queries), handler, workers })
    }

    pub fn send(&self, command: Command) -> Result<(), BackendError> {
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
        ClientCommand::ListSkills { .. }
            | ClientCommand::ListAgents { .. }
            | ClientCommand::ListMcpServers { .. }
            | ClientCommand::ListPermissions { .. }
            | ClientCommand::TakeNotices
            | ClientCommand::Feature { .. }
            | ClientCommand::ApproveMcpServer { .. }
            | ClientCommand::AddMcpServer { .. }
            | ClientCommand::RemoveMcpServer { .. }
            | ClientCommand::SignInMcpServer { .. }
            | ClientCommand::SignOutMcpServer { .. }
            | ClientCommand::OpenChannels { .. }
    )
}

fn spawn_worker(
    name: &str,
    commands: Receiver<Command>,
    handler: Arc<CommandHandler>,
) -> Result<JoinHandle<()>, BackendError> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            for command in commands {
                handler.handle(command);
            }
        })
        .map_err(BackendError::Worker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_routed_apart_from_run_control() {
        assert!(is_query(&ClientCommand::ListSkills { path: "/".into() }));
        assert!(is_query(&ClientCommand::ListAgents { path: "/".into() }));
        assert!(!is_query(&ClientCommand::ListModels));
    }
}
