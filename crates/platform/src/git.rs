use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::bounded::{Bounds, Ended};

const NOWHERE: &str = "/dev/null";

const SETTLED: [&str; 4] =
    ["core.fsmonitor=false", "core.pager=cat", "protocol.ext.allow=never", "diff.ignoreSubmodules=dirty"];

const DRIVER_COMMANDS: [&str; 3] = ["clean", "smudge", "process"];

const SCAN_WAIT: Duration = Duration::from_secs(10);
const SCAN_MOST: usize = 256 * 1024;

pub fn command(cwd: &Path) -> Command {
    let mut command = hardened(cwd);
    for driver in repository_drivers(cwd) {
        for run in DRIVER_COMMANDS {
            command.arg("-c").arg(format!("filter.{driver}.{run}="));
        }
        command.arg("-c").arg(format!("filter.{driver}.required=false"));
    }
    command
}

fn hardened(cwd: &Path) -> Command {
    let mut command = crate::command("git");
    command.args(["--no-pager", "--no-optional-locks"]);
    for setting in SETTLED {
        command.args(["-c", setting]);
    }
    command.arg("-c").arg(format!("core.hooksPath={NOWHERE}"));
    command.current_dir(cwd).env("GIT_TERMINAL_PROMPT", "0").env("GIT_OPTIONAL_LOCKS", "0");
    command
}

fn repository_drivers(cwd: &Path) -> BTreeSet<String> {
    let mut scan = hardened(cwd);
    scan.args(["config", "--show-scope", "--name-only", "--get-regexp", r"^filter\..*\.(clean|smudge|process)$"]);
    let never = AtomicBool::new(false);
    let bounds = Bounds { timeout: SCAN_WAIT, max_output: SCAN_MOST, stop: &never, input: None };
    match crate::bounded::run(&mut scan, bounds) {
        Ok(done) if matches!(done.ended, Ended::Exited(status) if status.success()) => drivers_in(&done.stdout),
        _ => BTreeSet::new(),
    }
}

fn drivers_in(listed: &str) -> BTreeSet<String> {
    listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(scope, _)| matches!(*scope, "local" | "worktree"))
        .filter_map(|(_, name)| {
            let named = name.strip_prefix("filter.")?;
            DRIVER_COMMANDS.iter().find_map(|run| named.strip_suffix(&format!(".{run}"))).map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
#[cfg(unix)]
#[path = "git_tests.rs"]
mod tests;
