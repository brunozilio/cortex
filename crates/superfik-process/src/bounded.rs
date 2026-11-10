//! A command run to its end within bounds: its output read up to a cap
//! (the rest drained so it never blocks), its whole process tree ended when
//! a stop flag is raised or its time is up.

use std::io::{self, Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::Tree;

const POLL: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub struct Bounds<'a> {
    pub timeout: Duration,
    /// Bytes kept from each of stdout and stderr.
    pub max_output: usize,
    pub stop: &'a AtomicBool,
    /// Written to the command's stdin, which is closed after it.
    pub input: Option<Vec<u8>>,
}

/// How the command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Exited(ExitStatus),
    Stopped,
    TimedOut,
}

#[derive(Debug)]
pub struct Finished {
    pub ended: Ended,
    pub stdout: String,
    pub stderr: String,
}

impl Finished {
    /// The exit status, when the command ran to its end.
    pub fn status(&self) -> Option<ExitStatus> {
        match self.ended {
            Ended::Exited(status) => Some(status),
            Ended::Stopped | Ended::TimedOut => None,
        }
    }
}

/// Starts `command` with piped stdout and stderr and waits for it within
/// `bounds`. Only starting it can fail; how it ended is in [`Finished`].
pub fn run(command: &mut Command, bounds: Bounds<'_>) -> io::Result<Finished> {
    let stdin = if bounds.input.is_some() { Stdio::piped() } else { Stdio::null() };
    command.stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut tree = Tree::spawn(command)?;
    let writer = match (bounds.input, tree.child().stdin.take()) {
        (Some(input), Some(mut stdin)) => Some(std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        })),
        _ => None,
    };
    let stdout = reader(tree.child().stdout.take(), bounds.max_output);
    let stderr = reader(tree.child().stderr.take(), bounds.max_output);
    let started = Instant::now();
    let ended = loop {
        match tree.child().try_wait() {
            Ok(Some(status)) => break Ended::Exited(status),
            Ok(None) if bounds.stop.load(Ordering::Relaxed) => break Ended::Stopped,
            Ok(None) if started.elapsed() > bounds.timeout => break Ended::TimedOut,
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => {
                tree.kill();
                return Err(error);
            }
        }
    };
    tree.kill();
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    Ok(Finished { ended, stdout, stderr })
}

fn reader(pipe: Option<impl Read + Send + 'static>, cap: usize) -> JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = (&mut pipe).take(cap as u64).read_to_end(&mut bytes);
            let _ = io::copy(&mut pipe, &mut io::sink());
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut command = crate::command("/bin/sh");
        command.args(["-c", script]);
        command
    }

    fn bounds(stop: &AtomicBool, timeout: Duration) -> Bounds<'_> {
        Bounds { timeout, max_output: 8, stop, input: None }
    }

    #[test]
    fn output_is_capped_and_the_exit_status_kept() {
        let stop = AtomicBool::new(false);
        let done = run(&mut sh("echo 0123456789abc; echo oops >&2; exit 3"), bounds(&stop, Duration::from_secs(5)))
            .expect("run");
        assert_eq!(done.stdout, "01234567");
        assert_eq!(done.stderr, "oops\n");
        assert_eq!(done.status().and_then(|s| s.code()), Some(3));
    }

    #[test]
    fn input_reaches_stdin() {
        let stop = AtomicBool::new(false);
        let mut with = bounds(&stop, Duration::from_secs(5));
        with.input = Some(b"hello".to_vec());
        assert_eq!(run(&mut sh("cat"), with).expect("run").stdout, "hello");
    }

    #[test]
    fn a_stop_or_the_timeout_ends_it() {
        let stopped = AtomicBool::new(true);
        assert_eq!(
            run(&mut sh("sleep 5"), bounds(&stopped, Duration::from_secs(5))).expect("run").ended,
            Ended::Stopped
        );
        let free = AtomicBool::new(false);
        let slow = run(&mut sh("sleep 5"), bounds(&free, Duration::from_millis(50))).expect("run");
        assert_eq!(slow.ended, Ended::TimedOut);
        assert!(slow.status().is_none());
    }
}
