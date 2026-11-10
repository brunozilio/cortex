//! This computer's shared backend (Unix): every terminal talks to one
//! process that keeps local models, embeddings and memory loaded once.

use std::time::Duration;

use superfik_backend::LocalServices;
use superfik_backend::daemon::{Daemon, DaemonDir, DaemonOptions};

/// How long the daemon stays up with no client.
pub const IDLE: Duration = Duration::from_secs(30 * 60);

/// Serves this computer's clients until idle; returns at once when a
/// daemon is already running. `build` names this build, so clients can
/// tell when the running daemon is older.
pub fn serve_daemon(build: &str) -> std::io::Result<()> {
    let dir = DaemonDir::system().ok_or_else(|| std::io::Error::other("no data folder"))?;
    Daemon::serve(&dir, LocalServices::detect(), DaemonOptions { idle: IDLE, build: build.to_owned() })
}

/// The running daemon's process id and build.
pub fn daemon_info() -> Option<(u32, String)> {
    DaemonDir::system()?.info()
}

/// Where the daemon writes what it prints.
pub fn daemon_log() -> Option<std::path::PathBuf> {
    DaemonDir::system().map(|dir| dir.0.join("daemon.log"))
}
