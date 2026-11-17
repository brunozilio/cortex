use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::Receiver;
use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use crate::child::Child;
use crate::sync::Locked;

const POLL: Duration = Duration::from_millis(20);
const FIRST_POLL: Duration = Duration::from_millis(1);
const READ_GRACE: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct Bounds<'a> {
    pub timeout: Duration,
    pub max_output: usize,
    pub stop: &'a AtomicBool,
    pub input: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct Limits {
    pub timeout: Duration,
    pub max_output: usize,
    pub input: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy)]
enum Until<'a> {
    Flag(&'a AtomicBool),
    Token(&'a CancellationToken),
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

#[derive(Debug)]
pub struct Captured {
    pub ended: Ended,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Captured {
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
    Ok(finished(capture(command, Limits { timeout, max_output, input }, Until::Flag(stop), keep)?))
}

pub fn run_cancellable(command: &mut Command, limits: Limits, cancel: &CancellationToken) -> io::Result<Finished> {
    Ok(finished(capture(command, limits, Until::Token(cancel), Keep::Start)?))
}

pub fn run_raw(command: &mut Command, bounds: Bounds<'_>) -> io::Result<Captured> {
    let Bounds { timeout, max_output, stop, input } = bounds;
    capture(command, Limits { timeout, max_output, input }, Until::Flag(stop), Keep::Start)
}

fn finished(Captured { ended, stdout, stderr }: Captured) -> Finished {
    let text = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned();
    Finished { ended, stdout: text(stdout), stderr: text(stderr) }
}

fn capture(command: &mut Command, limits: Limits, until: Until<'_>, keep: Keep) -> io::Result<Captured> {
    let Limits { timeout, max_output, input } = limits;
    let stdin = if input.is_some() { Stdio::piped() } else { Stdio::null() };
    command.stdin(stdin).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = Child::spawn(command)?;
    if let (Some(input), Some(mut stdin)) = (input, child.take_stdin()) {
        drop(crate::named::spawn("process-input", move || {
            let _ = stdin.write_all(&input);
        }));
    }
    let stdout = Reader::of(child.take_stdout(), max_output, keep);
    let stderr = Reader::of(child.take_stderr(), max_output, keep);
    let ended = match until {
        Until::Flag(stop) => Ok(wait(&child, timeout, stop)),
        Until::Token(cancel) => crate::runtime::block_on(waited(&child, timeout, cancel)).map_err(io::Error::other),
    };
    child.kill();
    let until = Instant::now() + READ_GRACE;
    Ok(Captured { ended: ended?, stdout: stdout.read_until(until), stderr: stderr.read_until(until) })
}

async fn waited(child: &Child, timeout: Duration, cancel: &CancellationToken) -> Ended {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut pause = FIRST_POLL;
    loop {
        if let Some(status) = child.try_wait() {
            return Ended::Exited(status);
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ended::TimedOut;
        }
        tokio::select! {
            biased;
            () = cancel.cancelled() => return Ended::Stopped,
            () = tokio::time::sleep(pause.min(deadline - now)) => {}
        }
        pause = (pause * 2).min(POLL);
    }
}

fn wait(child: &Child, timeout: Duration, stop: &AtomicBool) -> Ended {
    let started = Instant::now();
    let mut pause = FIRST_POLL;
    loop {
        match child.try_wait() {
            Some(status) => return Ended::Exited(status),
            None if stop.load(Ordering::Relaxed) => return Ended::Stopped,
            None if started.elapsed() > timeout => return Ended::TimedOut,
            None => {
                std::thread::sleep(pause);
                pause = (pause * 2).min(POLL);
            }
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
        let (end, ended) = crossbeam_channel::bounded(1);
        let into = Arc::clone(&kept);
        drop(crate::named::spawn("process-output", move || {
            if let Some(pipe) = pipe {
                copy(pipe, &into, cap, keep);
            }
            let _ = end.send(());
        }));
        Self { kept, ended }
    }

    fn read_until(self, until: Instant) -> Vec<u8> {
        let _ = self.ended.recv_timeout(until.saturating_duration_since(Instant::now()));
        self.kept.locked().iter().copied().collect()
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
    fn a_command_ended_at_its_timeout_leaves_no_zombie() {
        let dir = tempfile::tempdir().expect("dir");
        let pid_file = dir.path().join("pid");
        let free = AtomicBool::new(false);
        let script = format!("echo $$ > '{}'; exec sleep 30", pid_file.display());
        let slow = run(&mut sh(&script), bounds(&free, Duration::from_secs(1))).expect("run");
        assert_eq!(slow.ended, Ended::TimedOut);
        let pid: u32 = std::fs::read_to_string(&pid_file).expect("pid").trim().parse().expect("a pid");
        assert!(!crate::alive(pid), "pid {pid} was killed and never waited for");
    }

    #[test]
    fn output_that_is_not_text_is_kept_byte_for_byte() {
        let stop = AtomicBool::new(false);
        let mut with = bounds(&stop, Duration::from_secs(5));
        with.max_output = 64;
        let done = run_raw(&mut sh("printf '\\377\\000\\376'"), with).expect("run");
        assert_eq!(done.stdout, [0xff, 0x00, 0xfe]);
        assert!(done.status().is_some_and(|status| status.success()));
    }

    #[test]
    fn a_cancelled_token_ends_the_command_at_once_and_its_process_with_it() {
        let dir = tempfile::tempdir().expect("dir");
        let pid_file = dir.path().join("pid");
        let script = format!("echo $$ > '{}'; echo started; exec sleep 30", pid_file.display());
        let cancel = CancellationToken::new();
        let canceller = cancel.clone();
        let watched = pid_file.clone();
        let cancelled_at = std::thread::spawn(move || {
            while !watched.exists() {
                std::thread::sleep(Duration::from_millis(2));
            }
            std::thread::sleep(Duration::from_millis(50));
            let cancelled = Instant::now();
            canceller.cancel();
            cancelled
        });
        let limits = Limits { timeout: Duration::from_secs(60), max_output: 64, input: None };
        let done = run_cancellable(&mut sh(&script), limits, &cancel).expect("run");
        let ended = Instant::now();
        let took = ended.saturating_duration_since(cancelled_at.join().expect("cancelled"));
        eprintln!("cancel latency, a bounded command: {took:?}");
        assert_eq!(done.ended, Ended::Stopped);
        assert_eq!(done.stdout, "started\n");
        assert!(took < Duration::from_secs(1), "{took:?}");
        let pid: u32 = std::fs::read_to_string(&pid_file).expect("pid").trim().parse().expect("a pid");
        assert!(!crate::alive(pid), "pid {pid} was killed and waited for");
    }

    #[test]
    fn a_command_waited_on_with_a_token_keeps_its_output_status_and_timeout() {
        let free = CancellationToken::new();
        let limits = |timeout| Limits { timeout, max_output: 8, input: Some(b"in".to_vec()) };
        let done = run_cancellable(&mut sh("cat; echo 0123456789; exit 4"), limits(Duration::from_secs(5)), &free)
            .expect("run");
        assert_eq!(done.stdout, "in012345");
        assert_eq!(done.status().and_then(|status| status.code()), Some(4));
        let slow = run_cancellable(&mut sh("sleep 5"), limits(Duration::from_millis(50)), &free).expect("run");
        assert_eq!(slow.ended, Ended::TimedOut);
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let stopped = run_cancellable(&mut sh("sleep 5"), limits(Duration::from_secs(5)), &cancelled).expect("run");
        assert_eq!(stopped.ended, Ended::Stopped);
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
