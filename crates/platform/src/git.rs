use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::bounded::{Bounds, Ended};

const NOWHERE: &str = "/dev/null";

const SETTLED: [&str; 4] =
    ["core.fsmonitor=false", "core.pager=cat", "protocol.ext.allow=never", "diff.ignoreSubmodules=dirty"];

const READS_RUN_NOTHING: [&str; 1] = ["log.showSignature=false"];

const PLAIN_DIFF: &str = r#"f() { diff -u -- "$2" "$5"; true; }; f"#;
const PLAIN_TEXT: &str = "cat";

const DRIVER_COMMANDS: [&str; 3] = ["clean", "smudge", "process"];
const DIFF_COMMANDS: [&str; 2] = ["textconv", "command"];

const SCAN_WAIT: Duration = Duration::from_secs(10);
const SCAN_MOST: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Driver {
    section: &'static str,
    name: String,
}

pub fn command(cwd: &Path) -> Command {
    let mut command = hardened(cwd);
    for driver in repository_drivers(cwd).into_iter().filter(|driver| driver.section == "filter") {
        for setting in emptied(&driver) {
            command.arg("-c").arg(setting);
        }
    }
    command
}

pub fn environment(cwd: &Path, inherited: Option<&str>) -> Vec<(String, String)> {
    let mut settings: Vec<String> =
        SETTLED.iter().chain(&READS_RUN_NOTHING).map(|setting| (*setting).to_owned()).collect();
    settings.extend(repository_drivers(cwd).iter().flat_map(emptied));
    let first = inherited.and_then(|count| count.trim().parse::<usize>().ok()).unwrap_or(0);
    let mut environment = Vec::new();
    for (at, setting) in settings.iter().enumerate() {
        let (key, value) = setting.split_once('=').unwrap_or((setting, ""));
        environment.push((format!("GIT_CONFIG_KEY_{}", first + at), key.to_owned()));
        environment.push((format!("GIT_CONFIG_VALUE_{}", first + at), value.to_owned()));
    }
    environment.push(("GIT_CONFIG_COUNT".to_owned(), (first + settings.len()).to_string()));
    environment
}

fn emptied(driver: &Driver) -> Vec<String> {
    let Driver { section, name } = driver;
    if *section == "diff" {
        let harmless = if name.ends_with(".textconv") { PLAIN_TEXT } else { PLAIN_DIFF };
        return vec![format!("{name}={harmless}")];
    }
    let mut settings: Vec<String> = DRIVER_COMMANDS.iter().map(|run| format!("filter.{name}.{run}=")).collect();
    settings.push(format!("filter.{name}.required=false"));
    settings
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

fn repository_drivers(cwd: &Path) -> BTreeSet<Driver> {
    let mut scan = hardened(cwd);
    scan.args([
        "config",
        "--show-scope",
        "--name-only",
        "--get-regexp",
        r"^(filter\..*\.(clean|smudge|process)|diff\..*\.(textconv|command)|diff\.external)$",
    ]);
    let never = AtomicBool::new(false);
    let bounds = Bounds { timeout: SCAN_WAIT, max_output: SCAN_MOST, stop: &never, input: None };
    match crate::bounded::run(&mut scan, bounds) {
        Ok(done) if matches!(done.ended, Ended::Exited(status) if status.success()) => drivers_in(&done.stdout),
        _ => BTreeSet::new(),
    }
}

fn drivers_in(listed: &str) -> BTreeSet<Driver> {
    listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(scope, _)| matches!(*scope, "local" | "worktree"))
        .filter_map(|(_, name)| {
            if name == "diff.external" {
                return Some(Driver { section: "diff", name: name.to_owned() });
            }
            let rest = name.strip_prefix("filter.")?;
            let driver = DRIVER_COMMANDS.iter().find_map(|run| rest.strip_suffix(&format!(".{run}")));
            driver.map(|driver| Driver { section: "filter", name: driver.to_owned() })
        })
        .chain(listed.lines().filter_map(|line| {
            let (scope, name) = line.split_once('\t')?;
            let rest = name.strip_prefix("diff.").filter(|_| matches!(scope, "local" | "worktree"))?;
            let runs = DIFF_COMMANDS.iter().any(|run| rest.len() > run.len() + 1 && rest.ends_with(&format!(".{run}")));
            runs.then(|| Driver { section: "diff", name: name.to_owned() })
        }))
        .collect()
}

#[cfg(test)]
#[cfg(unix)]
#[path = "git_tests.rs"]
mod tests;
