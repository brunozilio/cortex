//! One backend per computer, shared by every terminal (Unix). The daemon
//! loads the heavy things once (local MLX models, the embedding model,
//! each project's memory) and serves each client over a Unix socket with
//! a [`LocalBackend`] of its own on the same services, so a client gets
//! only its own runs' events and answers, and its runs end with it.
//!
//! Messages are newline-delimited JSON: `Envelope<ClientCommand>` in,
//! `ServerEvent` out, the same values a remote backend will carry.

mod socket;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use protocol::{Backend, ClientCommand, Envelope};

pub use self::socket::{SocketBackend, connect_or_start};
use crate::{LocalBackend, LocalServices};

const SOCKET: &str = "daemon.sock";
const LOCK: &str = "daemon.lock";
const INFO: &str = "daemon.json";

/// Where a daemon listens, and what it says about itself.
#[derive(Debug, Clone)]
pub struct DaemonDir(pub PathBuf);

impl DaemonDir {
    pub fn system() -> Option<Self> {
        crate::local_data_dir().map(|dir| Self(dir.join("daemon")))
    }

    pub fn socket(&self) -> PathBuf {
        self.0.join(SOCKET)
    }

    /// The running daemon's process id and build; `None` when no daemon
    /// answers (one that was killed leaves its file behind).
    pub fn info(&self) -> Option<(u32, String)> {
        if !is_listening(&self.socket()) {
            return None;
        }
        let text = std::fs::read_to_string(self.0.join(INFO)).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        Some((u32::try_from(value["pid"].as_u64()?).ok()?, value["build"].as_str()?.to_owned()))
    }
}

#[derive(Debug, Clone)]
pub struct DaemonOptions {
    /// Exits after this long with no client.
    pub idle: Duration,
    /// Which build this is, so a client can tell an older daemon.
    pub build: String,
}

/// A running daemon; [`Daemon::stop`] ends it.
#[derive(Debug)]
pub struct Daemon {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Daemon {
    /// Starts listening in `dir`; `None` when another daemon holds it.
    pub fn start(dir: &DaemonDir, services: LocalServices, options: DaemonOptions) -> std::io::Result<Option<Self>> {
        std::fs::create_dir_all(&dir.0)?;
        std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o700))?;
        let lock = std::fs::File::create(dir.0.join(LOCK))?;
        if lock.try_lock().is_err() {
            return Ok(None);
        }
        let socket = dir.socket();
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let info = serde_json::json!({ "pid": std::process::id(), "build": options.build });
        std::fs::write(dir.0.join(INFO), info.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, dir) = (Arc::clone(&stop), dir.clone());
        let thread = std::thread::spawn(move || {
            accept(&listener, &services, &options, &flag);
            let _ = std::fs::remove_file(dir.socket());
            let _ = std::fs::remove_file(dir.0.join(INFO));
            drop(lock);
        });
        Ok(Some(Self { stop, thread: Some(thread) }))
    }

    /// Serves until idle; returns at once when another daemon is running.
    pub fn serve(dir: &DaemonDir, services: LocalServices, options: DaemonOptions) -> std::io::Result<()> {
        if let Some(mut daemon) = Self::start(dir, services, options)? {
            daemon.join();
        }
        Ok(())
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.join();
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("the daemon's listener panicked");
        }
    }
}

fn accept(listener: &UnixListener, services: &LocalServices, options: &DaemonOptions, stop: &AtomicBool) {
    let clients = Arc::new(AtomicUsize::new(0));
    let mut alone_since = Some(Instant::now());
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let (services, clients) = (services.clone(), Arc::clone(&clients));
                clients.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    if let Err(error) = serve_client(stream, services) {
                        tracing::warn!(%error, "a daemon client ended badly");
                    }
                    clients.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => tracing::warn!(%error, "the daemon could not accept a client"),
        }
        match (clients.load(Ordering::SeqCst), alone_since) {
            (0, None) => alone_since = Some(Instant::now()),
            (0, Some(since)) if since.elapsed() >= options.idle => return,
            (0, Some(_)) => {}
            _ => alone_since = None,
        }
    }
}

/// One client: its commands go to its own backend, that backend's events
/// back to it. Its runs are cancelled when it leaves.
fn serve_client(stream: UnixStream, services: LocalServices) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    let (wake_tx, woken) = crossbeam_channel::bounded::<()>(1);
    let wake = Arc::new(move || {
        let _ = wake_tx.try_send(());
    });
    let backend = Arc::new(LocalBackend::new(services, wake).map_err(std::io::Error::other)?);
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    let done = Arc::new(AtomicBool::new(false));
    let sender = {
        let (backend, writer, done) = (Arc::clone(&backend), Arc::clone(&writer), Arc::clone(&done));
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let _ = woken.recv_timeout(Duration::from_millis(250));
                while let Some(event) = backend.try_recv() {
                    if write_line(&writer, &event).is_err() {
                        return;
                    }
                }
            }
        })
    };
    for line in BufReader::new(stream).lines() {
        let line = line?;
        match serde_json::from_str::<Envelope<ClientCommand>>(&line) {
            Ok(command) => backend.send(command).map_err(std::io::Error::other)?,
            Err(error) => tracing::warn!(%error, "the daemon got a message it does not understand"),
        }
    }
    done.store(true, Ordering::Relaxed);
    let _ = sender.join();
    Ok(())
}

fn write_line<T: serde::Serialize>(writer: &Mutex<UnixStream>, value: &T) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    line.push(b'\n');
    let mut writer = writer.lock().unwrap_or_else(PoisonError::into_inner);
    writer.write_all(&line)?;
    writer.flush()
}

/// Whether a path is a socket some daemon answers on.
pub fn is_listening(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

#[cfg(test)]
mod tests;
