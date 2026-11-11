//! How the CLI reaches its backend: this computer's shared daemon (Unix),
//! started in the background on first use, or one in this process
//! (`SUPERFIK_NO_DAEMON=1`, or Windows).

use sdk::Superfik;

/// This build: the version and when the binary was written, so a client
/// can tell that the running daemon is an older `superfik`.
pub fn build() -> String {
    let written = std::env::current_exe()
        .and_then(std::fs::metadata)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs());
    format!("{}+{written}", env!("CARGO_PKG_VERSION"))
}

/// The backend, and a warning when the daemon answering is an older build.
pub fn connect() -> Result<(Superfik, Option<String>), String> {
    #[cfg(unix)]
    if std::env::var_os("SUPERFIK_NO_DAEMON").is_none() {
        let superfik = Superfik::connect_daemon(start).map_err(|e| e.to_string())?;
        let stale = sdk::daemon::daemon_info().filter(|(_, running)| *running != build()).map(|_| {
            "The background daemon is from an older build: run `superfik daemon stop` to update it.".to_owned()
        });
        return Ok((superfik, stale));
    }
    Ok((Superfik::builder().build().map_err(|e| e.to_string())?, None))
}

#[cfg(unix)]
fn start() -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    let log = sdk::daemon::daemon_log().ok_or_else(|| std::io::Error::other("no data folder"))?;
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let log = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
    std::process::Command::new(std::env::current_exe()?)
        .args(["daemon", "serve"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log)
        .process_group(0)
        .spawn()
        .map(|_| ())
}

#[cfg(unix)]
pub fn serve() -> Result<(), String> {
    sdk::daemon::serve_daemon(&build()).map_err(|e| e.to_string())
}

#[cfg(unix)]
pub fn stop() -> Result<(), String> {
    match sdk::daemon::daemon_info() {
        Some((pid, _)) => {
            let status = std::process::Command::new("kill").arg(pid.to_string()).status().map_err(|e| e.to_string())?;
            if status.success() { Ok(()) } else { Err(format!("could not stop the daemon ({pid})")) }
        }
        None => Ok(()),
    }
}

#[cfg(unix)]
pub fn status() -> String {
    match sdk::daemon::daemon_info() {
        Some((pid, running)) if running == build() => format!("running (pid {pid})"),
        Some((pid, running)) => {
            format!("running an older build {running} (pid {pid}); `superfik daemon stop` updates it")
        }
        None => "not running".to_owned(),
    }
}
