//! Running a child process with a deadline, so a hung network call never
//! stalls the workspace worker.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(20);

pub fn output_within(mut command: Command, cwd: &Path, timeout: Duration) -> Option<String> {
    let mut child =
        command.current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut out = String::new();
        stdout.read_to_string(&mut out).map(|_| out).ok()
    });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return reader.join().ok().flatten(),
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn returns_stdout_of_successful_commands() {
        let out = output_within(sh("echo hi"), Path::new("/"), Duration::from_secs(5));
        assert_eq!(out.as_deref(), Some("hi\n"));
    }

    #[test]
    fn failures_and_missing_programs_are_none() {
        assert_eq!(output_within(sh("echo x; exit 1"), Path::new("/"), Duration::from_secs(5)), None);
        assert_eq!(output_within(Command::new("/definitely/not/gh"), Path::new("/"), Duration::from_secs(5)), None);
    }

    #[test]
    fn slow_commands_are_killed() {
        let started = Instant::now();
        assert_eq!(output_within(sh("exec sleep 5"), Path::new("/"), Duration::from_millis(100)), None);
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
