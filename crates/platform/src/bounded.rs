use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::Tree;
use crate::sync::Locked;

const POLL: Duration = Duration::from_millis(20);
const READ_GRACE: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct Bounds<'a> {
    pub timeout: Duration,
    pub max_output: usize,
    pub stop: &'a AtomicBool,
    pub input: Option<Vec<u8>>,
}

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
    pub fn status(&self) -> Option<ExitStatus> {
        match self.ended {
            Ended::Exited(status) => Some(status),
            Ended::Stopped | Ended::TimedOut => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    Start,
    End,
}

pub fn run(command: &mut Command, bounds: Bounds<'_>) -> io::Result<Finished> {
    run_keeping(command, bounds, Keep::Start)
}

pub fn run_keeping(command: &mut Command, bounds: Bounds<'_>, keep: Keep) -> io::Result<Finished> {
    let Bounds { timeout, max_output, stop, input } = bounds;
    let stdin = if input.is_some() { Stdio::piped() } else { Stdio::null() };
    command.stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut tree = Tree::spawn(command)?;
    if let (Some(input), Some(mut stdin)) = (input, tree.child().stdin.take()) {
        drop(crate::named::spawn("process-input", move || {
            let _ = stdin.write_all(&input);
        }));
    }
    let stdout = Reader::of(tree.child().stdout.take(), max_output, keep);
    let stderr = Reader::of(tree.child().stderr.take(), max_output, keep);
    let ended = wait(&mut tree, timeout, stop);
    tree.kill();
    let ended = ended?;
    let until = Instant::now() + READ_GRACE;
    Ok(Finished { ended, stdout: stdout.read_until(until), stderr: stderr.read_until(until) })
}

fn wait(tree: &mut Tree, timeout: Duration, stop: &AtomicBool) -> io::Result<Ended> {
    let started = Instant::now();
    loop {
        match tree.child().try_wait()? {
            Some(status) => return Ok(Ended::Exited(status)),
            None if stop.load(Ordering::Relaxed) => return Ok(Ended::Stopped),
            None if started.elapsed() > timeout => return Ok(Ended::TimedOut),
            None => std::thread::sleep(POLL),
        }
    }
}

struct Reader {
    kept: Arc<Mutex<VecDeque<u8>>>,
    ended: Receiver<()>,
}

impl Reader {
    fn of(pipe: Option<impl Read + Send + 'static>, cap: usize, keep: Keep) -> Self {
        let kept = Arc::new(Mutex::new(VecDeque::new()));
        let (end, ended) = std::sync::mpsc::channel();
        let into = Arc::clone(&kept);
        drop(crate::named::spawn("process-output", move || {
            if let Some(pipe) = pipe {
                copy(pipe, &into, cap, keep);
            }
            let _ = end.send(());
        }));
        Self { kept, ended }
    }

    fn read_until(self, until: Instant) -> String {
        let _ = self.ended.recv_timeout(until.saturating_duration_since(Instant::now()));
        let bytes: Vec<u8> = self.kept.locked().iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

fn copy(mut pipe: impl Read, into: &Mutex<VecDeque<u8>>, cap: usize, keep: Keep) {
    let mut chunk = [0u8; 8192];
    loop {
        let read = match pipe.read(&mut chunk) {
            Ok(0) => return,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        let mut kept = into.locked();
        match keep {
            Keep::Start => {
                let room = cap.saturating_sub(kept.len()).min(read);
                kept.extend(&chunk[..room]);
            }
            Keep::End => {
                kept.extend(&chunk[..read]);
                let over = kept.len().saturating_sub(cap);
                kept.drain(..over);
            }
        }
    }
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
    fn a_grandchild_that_left_the_group_cannot_hold_the_run_open() {
        let stop = AtomicBool::new(false);
        let leaves = "if command -v setsid >/dev/null 2>&1; then setsid sleep 10 & else set -m; sleep 10 & fi; echo hi";
        let started = Instant::now();
        let done = run(&mut sh(leaves), bounds(&stop, Duration::from_secs(3))).expect("run");
        assert!(started.elapsed() < Duration::from_secs(6), "held open for {:?}", started.elapsed());
        assert_eq!(done.stdout, "hi\n");
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
