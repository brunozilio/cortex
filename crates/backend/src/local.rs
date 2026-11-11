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
    memory: Option<Sender<Command>>,
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
        let (memory, memory_rx) = crossbeam_channel::unbounded();
        let workers = vec![
            spawn_worker("backend-control", control_rx, Arc::clone(&handler))?,
            spawn_worker("backend-queries", queries_rx, Arc::clone(&handler))?,
            spawn_memory_worker(memory_rx, Arc::clone(&handler))?,
        ];
        Ok(Self { control: Some(control), queries: Some(queries), memory: Some(memory), handler, workers })
    }

    pub fn send(&self, command: Command) -> Result<(), BackendError> {
        let queue = if is_memory(&command.payload) {
            &self.memory
        } else if is_query(&command.payload) {
            &self.queries
        } else {
            &self.control
        };
        queue.as_ref().ok_or(BackendError::Disconnected)?.send(command).map_err(|_| BackendError::Disconnected)
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        self.control.take();
        self.queries.take();
        self.memory.take();
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

fn is_memory(command: &ClientCommand) -> bool {
    matches!(
        command,
        ClientCommand::LoadMemory { .. }
            | ClientCommand::EditMemory { .. }
            | ClientCommand::SearchMemory { .. }
            | ClientCommand::UndoMemory { .. }
    )
}

fn coalesce(pending: Vec<Command>) -> Vec<Command> {
    let last_search = |path: &std::path::Path| {
        pending.iter().rposition(|c| matches!(&c.payload, ClientCommand::SearchMemory { path: p, .. } if p == path))
    };
    pending
        .iter()
        .enumerate()
        .filter(|(i, c)| match &c.payload {
            ClientCommand::SearchMemory { path, .. } => last_search(path) == Some(*i),
            _ => true,
        })
        .map(|(_, c)| c.clone())
        .collect()
}

fn spawn_memory_worker(
    commands: Receiver<Command>,
    handler: Arc<CommandHandler>,
) -> Result<JoinHandle<()>, BackendError> {
    thread::Builder::new()
        .name("backend-memory".to_owned())
        .spawn(move || {
            while let Ok(first) = commands.recv() {
                let pending: Vec<Command> = std::iter::once(first).chain(commands.try_iter()).collect();
                for command in coalesce(pending) {
                    handler.handle(command);
                }
            }
        })
        .map_err(BackendError::Worker)
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
        assert!(is_memory(&ClientCommand::SearchMemory { path: "/".into(), query: "x".into() }));
        assert!(!is_memory(&ClientCommand::ListModels));
    }

    fn search(path: &str, query: &str) -> Command {
        Envelope::local(ClientCommand::SearchMemory { path: path.into(), query: query.into() })
    }

    #[test]
    fn only_the_latest_search_per_project_runs() {
        let load = Envelope::local(ClientCommand::LoadMemory { path: "/a".into() });
        let kept = coalesce(vec![search("/a", "r"), search("/b", "x"), load, search("/a", "re"), search("/a", "ren")]);
        let names: Vec<String> = kept
            .iter()
            .map(|c| match &c.payload {
                ClientCommand::SearchMemory { path, query } => format!("{}:{query}", path.display()),
                other => other.name().to_owned(),
            })
            .collect();
        assert_eq!(names, vec!["/b:x", "load_memory", "/a:ren"]);
    }
}
