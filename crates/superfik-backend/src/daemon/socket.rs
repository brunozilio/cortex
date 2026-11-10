//! The client's side: a [`Backend`] that is a socket to the daemon.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use superfik_protocol::{Backend, BackendError, ClientCommand, Envelope, ServerEvent, Wake};

use super::{DaemonDir, write_line};

pub struct SocketBackend {
    writer: Arc<Mutex<UnixStream>>,
    events: Receiver<ServerEvent>,
}

impl std::fmt::Debug for SocketBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SocketBackend").field("pending_events", &self.events.len()).finish_non_exhaustive()
    }
}

impl SocketBackend {
    pub fn connect(dir: &DaemonDir, wake: Wake) -> std::io::Result<Self> {
        let stream = UnixStream::connect(dir.socket())?;
        let reader = stream.try_clone()?;
        let (tx, events) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<ServerEvent>(&line) {
                    Ok(event) => {
                        if tx.send(event).is_err() {
                            return;
                        }
                        wake();
                    }
                    Err(error) => tracing::warn!(%error, "the daemon sent something this client does not understand"),
                }
            }
            let _ = tx.send(ServerEvent::error("The Superfik daemon stopped. Start superfik again."));
            wake();
        });
        Ok(Self { writer: Arc::new(Mutex::new(stream)), events })
    }
}

impl Backend for SocketBackend {
    fn send(&self, command: Envelope<ClientCommand>) -> Result<(), BackendError> {
        write_line(&self.writer, &command).map_err(|_| BackendError::Disconnected)
    }

    fn try_recv(&self) -> Option<ServerEvent> {
        self.events.try_recv().ok()
    }
}

impl Drop for SocketBackend {
    /// Leaving closes the connection (its reader holds a copy), so the
    /// daemon ends this client's runs.
    fn drop(&mut self) {
        let writer = self.writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = writer.shutdown(std::net::Shutdown::Both);
    }
}

/// Connects to the daemon in `dir`, starting one with `start` (which
/// launches it in the background) when none answers.
pub fn connect_or_start(
    dir: &DaemonDir,
    wake: Wake,
    start: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<SocketBackend> {
    if let Ok(backend) = SocketBackend::connect(dir, Arc::clone(&wake)) {
        return Ok(backend);
    }
    start()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match SocketBackend::connect(dir, Arc::clone(&wake)) {
            Ok(backend) => return Ok(backend),
            Err(error) if Instant::now() >= deadline => {
                return Err(std::io::Error::other(format!("the Superfik daemon did not start: {error}")));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}
