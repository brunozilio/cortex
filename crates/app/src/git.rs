use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::{Bounds, Ended};

pub const LOCAL: Duration = Duration::from_secs(30);
pub const REMOTE: Duration = Duration::from_secs(120);
const MOST_OUTPUT: usize = 16 << 20;

pub fn run(cwd: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    run_program(OsStr::new("git"), cwd, args, limit)
}

fn run_program(program: &OsStr, cwd: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut command = platform::command(program);
    command.args(args).current_dir(cwd).env("GIT_TERMINAL_PROMPT", "0");
    let never = AtomicBool::new(false);
    let bounds = Bounds { timeout: limit, max_output: MOST_OUTPUT, stop: &never, input: None };
    let finished = platform::bounded::run(&mut command, bounds).map_err(|error| format!("git: {error}"))?;
    match finished.ended {
        Ended::Exited(status) if status.success() => Ok(finished.stdout),
        Ended::Exited(_) => Err(finished.stderr.trim().to_owned()),
        Ended::TimedOut | Ended::Stopped => Err(format!(
            "git {} did not finish within {} seconds and was stopped",
            args.iter().find(|arg| !arg.starts_with('-')).copied().unwrap_or_default(),
            limit.as_secs()
        )),
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn script(dir: &Path, body: &str) -> String {
        let path = dir.join("fake-git.sh");
        std::fs::write(&path, body).expect("script");
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_git_that_never_answers_is_stopped_at_the_limit() {
        let dir = tempfile::tempdir().expect("dir");
        let hangs = script(dir.path(), "sleep 30\n");
        let asked = Instant::now();
        let stopped = run_program(OsStr::new("/bin/sh"), dir.path(), &[&hangs, "clone"], Duration::from_millis(200))
            .expect_err("stopped");
        assert!(asked.elapsed() < Duration::from_secs(10), "waited {:?}", asked.elapsed());
        assert!(stopped.contains("did not finish"), "{stopped}");
    }

    #[test]
    fn git_cannot_prompt_and_has_nothing_to_read() {
        let dir = tempfile::tempdir().expect("dir");
        let asks = script(dir.path(), "printf 'prompt=%s|' \"$GIT_TERMINAL_PROMPT\"\ncat\necho read-nothing\n");
        let said = run_program(OsStr::new("/bin/sh"), dir.path(), &[&asks], LOCAL).expect("ran");
        assert_eq!(said, "prompt=0|read-nothing\n");
    }

    #[test]
    fn a_failure_is_what_git_wrote_to_its_error_stream() {
        let dir = tempfile::tempdir().expect("dir");
        let refused = run(dir.path(), &["rev-parse", "--verify", "HEAD"], LOCAL).expect_err("not a repository");
        assert!(refused.contains("not a git repository"), "{refused}");
    }
}
