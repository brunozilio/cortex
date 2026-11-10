//! Bounded `git` calls: a clean environment, capped output, cancelled by
//! the run's stop flag or after thirty seconds.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use superfik_process::{Bounds, Ended};

use crate::tools::Workspace;

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OUTPUT: usize = 128 * 1024;

/// Git's output, trimmed.
pub(crate) fn git(cwd: &Path, args: &[&OsStr]) -> Result<String, String> {
    output(cwd, args, &AtomicBool::new(false)).map(|out| out.trim().to_owned())
}

pub(crate) fn git_stopped(workspace: &Workspace, cwd: &Path, args: &[&OsStr]) -> Result<String, String> {
    output(cwd, args, &workspace.stop).map(|out| out.trim().to_owned())
}

/// Git's output as printed.
pub(crate) fn output(cwd: &Path, args: &[&OsStr], stop: &AtomicBool) -> Result<String, String> {
    let mut command = superfik_process::command("git");
    command.arg("-C").arg(cwd).args(args);
    let bounds = Bounds { timeout: TIMEOUT, max_output: MAX_OUTPUT, stop, input: None };
    let done = superfik_process::bounded::run(&mut command, bounds).map_err(|e| e.to_string())?;
    match done.ended {
        Ended::Exited(status) if status.success() => Ok(done.stdout),
        Ended::Exited(_) => Err(format!("git failed: {}", done.stderr)),
        Ended::Stopped | Ended::TimedOut => Err("git operation cancelled or timed out".into()),
    }
}
