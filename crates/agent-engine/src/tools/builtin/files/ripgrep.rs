use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::{Bounds, Ended};

const WAIT: Duration = Duration::from_secs(120);
const LISTED_AT_MOST: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct Asked<'a> {
    pub(super) pattern: &'a str,
    pub(super) ignore_case: bool,
    pub(super) multiline: bool,
}

pub fn program() -> Option<PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND.get_or_init(|| on_path(std::env::var_os("PATH").as_deref())).clone()
}

pub(super) fn on_path(path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let name = if cfg!(windows) { "rg.exe" } else { "rg" };
    std::env::split_paths(path?).map(|folder| folder.join(name)).find(|candidate| candidate.is_file())
}

pub(super) fn arguments(root: &Path, asked: &Asked<'_>) -> Vec<std::ffi::OsString> {
    let mut arguments: Vec<std::ffi::OsString> = [
        "--no-config",
        "--files-with-matches",
        "--null",
        "--hidden",
        "--text",
        "--no-messages",
        "--color=never",
        "--glob=!.git",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    if asked.ignore_case {
        arguments.push("--ignore-case".into());
    }
    if asked.multiline {
        arguments.extend(["--multiline".into(), "--multiline-dotall".into()]);
    }
    arguments.extend(["--regexp".into(), asked.pattern.into(), "--".into(), root.as_os_str().to_owned()]);
    arguments
}

pub(super) fn candidates(program: &Path, root: &Path, asked: &Asked<'_>, stop: &AtomicBool) -> Option<Vec<PathBuf>> {
    let mut command = std::process::Command::new(program);
    command.args(arguments(root, asked)).stdin(std::process::Stdio::null());
    let bounds = Bounds { timeout: WAIT, max_output: LISTED_AT_MOST, stop, input: None };
    let done = platform::bounded::run_raw(&mut command, bounds).ok()?;
    let code = match done.ended {
        Ended::Exited(status) => status.code(),
        Ended::Stopped | Ended::TimedOut => return None,
    };
    let listed = done.stdout.len() < LISTED_AT_MOST;
    match code {
        Some(0 | 1) if listed => Some(paths(&done.stdout)),
        Some(2) if listed && !done.stdout.is_empty() => Some(paths(&done.stdout)),
        _ => {
            tracing::debug!(?code, "ripgrep did not answer; the built-in search is used");
            None
        }
    }
}

fn paths(listed: &[u8]) -> Vec<PathBuf> {
    listed.split(|byte| *byte == 0).filter(|part| !part.is_empty()).map(path_of).collect()
}

#[cfg(unix)]
fn path_of(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_of(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
