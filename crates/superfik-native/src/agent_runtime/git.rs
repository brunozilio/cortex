//! Bounded `git` calls for agent setup: a clean environment, capped
//! output, cancelled by the run's stop flag or after thirty seconds.

use std::{io::Read, path::Path};

use crate::tools::Workspace;

pub(super) fn git(cwd: &Path, args: &[&std::ffi::OsStr]) -> Result<String, String> {
    git_impl(cwd, args, &std::sync::atomic::AtomicBool::new(false))
}
pub(super) fn git_stopped(workspace: &Workspace, cwd: &Path, args: &[&std::ffi::OsStr]) -> Result<String, String> {
    git_impl(cwd, args, &workspace.stop)
}
fn git_impl(cwd: &Path, args: &[&std::ffi::OsStr], stop: &std::sync::atomic::AtomicBool) -> Result<String, String> {
    use std::{
        process::Stdio,
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };
    let mut cmd = superfik_process::command("git");
    cmd.arg("-C").arg(cwd).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut tree = superfik_process::Tree::spawn(&mut cmd).map_err(|e| e.to_string())?;
    let output = git_reader(tree.child().stdout.take());
    let error = git_reader(tree.child().stderr.take());
    let started = Instant::now();
    let status = loop {
        if let Some(status) = tree.child().try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if stop.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(30) {
            tree.kill();
            let _ = output.join();
            let _ = error.join();
            return Err("git operation cancelled or timed out".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    tree.kill();
    let output = output.join().unwrap_or_default();
    let error = error.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("git failed: {error}"));
    }
    Ok(output.trim().to_owned())
}
fn git_reader(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = (&mut pipe).take(128 * 1024).read_to_end(&mut bytes);
            let _ = std::io::copy(&mut pipe, &mut std::io::sink());
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}
