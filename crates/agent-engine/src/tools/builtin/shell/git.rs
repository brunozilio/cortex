use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::Bounds;

const WAIT: Duration = Duration::from_secs(10);
pub(super) const LISTED_AT_MOST: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct Git<'a> {
    pub(super) stop: &'a AtomicBool,
    pub(super) wait: Duration,
}

impl<'a> Git<'a> {
    pub(super) fn stopped_by(stop: &'a AtomicBool) -> Self {
        Self { stop, wait: WAIT }
    }

    pub(super) fn bytes(&self, cwd: &Path, args: &[&str], most: usize) -> Option<Vec<u8>> {
        let mut command = platform::git::command(cwd);
        command.args(["-c", "core.quotepath=off"]).args(args);
        let bounds = Bounds { timeout: self.wait, max_output: most, stop: self.stop, input: None };
        let done = platform::bounded::run_raw(&mut command, bounds).ok()?;
        done.status().is_some_and(|status| status.success()).then_some(done.stdout)
    }

    pub(super) fn text(&self, cwd: &Path, args: &[&str]) -> Option<String> {
        self.bytes(cwd, args, LISTED_AT_MOST).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    pub(super) fn status(&self, root: &Path, more: &[&str]) -> Option<Vec<u8>> {
        let mut args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all"];
        args.extend(more);
        self.bytes(root, &args, LISTED_AT_MOST)
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_git_command_gives_up_when_the_run_stops_or_its_time_is_over() {
        let dir = tempfile::tempdir().expect("dir");
        let hook = dir.path().join("slow");
        std::fs::write(&hook, "#!/bin/sh\nsleep 60\n").expect("script");
        std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("mode");
        let slow = format!("alias.slow=!{}", hook.display());
        let args = ["-c", slow.as_str(), "slow"];

        let stopped = AtomicBool::new(true);
        let started = Instant::now();
        assert_eq!(Git::stopped_by(&stopped).bytes(dir.path(), &args, 64), None);
        let free = AtomicBool::new(false);
        let brief = Git { stop: &free, wait: Duration::from_millis(100) };
        assert_eq!(brief.bytes(dir.path(), &args, 64), None);
        assert!(started.elapsed() < Duration::from_secs(30), "git ran on for {:?}", started.elapsed());

        let version = Git::stopped_by(&free).text(dir.path(), &["--version"]).expect("git");
        assert!(version.starts_with("git version"), "{version}");
    }
}
