//! A command `Bash` runs in the foreground waits here: until it exits, its
//! time is up, the run is cancelled, or the user moves it to the background
//! (Ctrl+B), where it goes on like one started with `run_in_background`.

use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Shell, lock, refresh};

/// How the wait ended, with what the command printed (stdout and stderr
/// as they came).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Waited {
    Exited {
        output: String,
        success: bool,
        code: Option<i32>,
    },
    /// Moved to the background; it goes on under its id.
    Detached,
    TimedOut(String),
    Stopped(String),
}

pub(crate) fn wait(id: &str, timeout: Duration, stop: &AtomicBool, detach: &AtomicBool) -> Waited {
    let started = Instant::now();
    // A Ctrl+B from before this command does not move it.
    detach.store(false, Ordering::SeqCst);
    loop {
        let exited = {
            let mut shells = lock();
            let Some(shell) = shells.get_mut(id) else { return Waited::Stopped(String::new()) };
            if refresh(shell).is_some() {
                let status = shell.tree.child().try_wait().ok().flatten();
                Some((status, shells.remove(id)))
            } else {
                None
            }
        };
        // Waiting for the readers holds no lock: other shells go on.
        if let Some((status, shell)) = exited {
            return Waited::Exited {
                output: taken(shell),
                success: status.is_some_and(|status| status.success()),
                code: status.and_then(|status| status.code()),
            };
        }
        if detach.swap(false, Ordering::SeqCst) {
            return Waited::Detached;
        }
        let stopped = stop.load(Ordering::Relaxed);
        if stopped || started.elapsed() > timeout {
            let mut shell = lock().remove(id);
            if let Some(shell) = &mut shell {
                shell.tree.kill();
                let _ = shell.tree.child().wait();
            }
            let output = taken(shell);
            return if stopped { Waited::Stopped(output) } else { Waited::TimedOut(output) };
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// What the shell printed, once its readers have caught up.
fn taken(shell: Option<Shell>) -> String {
    let Some(shell) = shell else { return String::new() };
    // The readers finish when the pipes close, just after the exit (or
    // never, when something the command left behind holds them).
    let deadline = Instant::now() + Duration::from_millis(500);
    while !shell.readers.iter().all(std::thread::JoinHandle::is_finished) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let kept = shell.output.lock().unwrap_or_else(PoisonError::into_inner);
    String::from_utf8_lossy(&kept).trim_end().to_owned()
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Stdio;

    use super::*;

    fn started(script: &str, client: u64) -> String {
        let mut command = std::process::Command::new("/bin/sh");
        command.arg("-c").arg(script).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        super::super::start(client, script, &mut command).expect("start")
    }

    #[test]
    fn a_command_is_waited_for_and_its_output_kept() {
        let (stop, detach) = (AtomicBool::new(false), AtomicBool::new(false));
        let id = started("echo out; echo err >&2; exit 3", 911);
        let waited = wait(&id, Duration::from_secs(5), &stop, &detach);
        let Waited::Exited { output, success, code } = waited else { panic!("{waited:?}") };
        assert!(output.contains("out") && output.contains("err"), "{output}");
        assert_eq!((success, code), (false, Some(3)));
        assert!(super::super::list(911).is_empty(), "an ended command leaves the list");
    }

    #[test]
    fn moved_to_the_background_it_goes_on_and_a_slow_one_times_out() {
        let stop = AtomicBool::new(false);
        let detach = std::sync::Arc::new(AtomicBool::new(true));
        let id = started("sleep 30", 912);
        let pressed = std::sync::Arc::clone(&detach);
        let ctrl_b = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            pressed.store(true, Ordering::SeqCst);
        });
        assert_eq!(wait(&id, Duration::from_secs(5), &stop, &detach), Waited::Detached, "the stale press is ignored");
        ctrl_b.join().expect("ctrl+b");
        assert_eq!(super::super::list(912), [(id.clone(), "sleep 30".to_owned(), true)]);
        assert!(!super::super::kill(999, &id), "another client cannot stop it");
        assert!(super::super::kill(912, &id));
        assert!(super::super::list(912).is_empty());

        let id = started("echo started; sleep 30", 913);
        let waited = wait(&id, Duration::from_millis(300), &stop, &AtomicBool::new(false));
        assert!(matches!(&waited, Waited::TimedOut(output) if output.contains("started")), "{waited:?}");
    }
}
