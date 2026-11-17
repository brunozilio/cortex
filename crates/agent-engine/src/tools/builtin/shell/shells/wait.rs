use platform::child::GRACE;
use platform::sync::Locked;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Shell, lock, refresh};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Waited {
    Exited { output: String, success: bool, code: Option<i32> },
    Detached,
    TimedOut(String),
    Stopped(String),
}

pub(crate) fn wait(id: &str, timeout: Duration, stop: &AtomicBool, detach: &AtomicBool) -> Waited {
    let started = Instant::now();
    detach.store(false, Ordering::SeqCst);
    let mut seen = 0;
    loop {
        let exited = {
            let mut shells = lock();
            let Some(shell) = shells.get_mut(id) else { return Waited::Stopped(String::new()) };
            let output = shell.output.locked();
            if output.len() != seen {
                seen = output.len();
                let tail = &output[output.len().saturating_sub(4096)..];
                crate::tools::progress::report(&String::from_utf8_lossy(tail));
            }
            drop(output);
            if refresh(shell).is_some() {
                let status = shell.child.try_wait();
                Some((status, shells.remove(id)))
            } else {
                None
            }
        };
        if let Some((status, shell)) = exited {
            return Waited::Exited {
                output: taken(shell.as_ref()),
                success: status.is_some_and(|status| status.success()),
                code: status.and_then(|status| status.code()),
            };
        }
        if detach.swap(false, Ordering::SeqCst) {
            return Waited::Detached;
        }
        let stopped = stop.load(Ordering::Relaxed);
        if stopped || started.elapsed() > timeout {
            let shell = lock().remove(id);
            if let Some(shell) = &shell {
                shell.child.terminate(GRACE);
            }
            let output = taken(shell.as_ref());
            return if stopped { Waited::Stopped(output) } else { Waited::TimedOut(output) };
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn taken(shell: Option<&Shell>) -> String {
    let Some(shell) = shell else { return String::new() };
    let deadline = Instant::now() + Duration::from_millis(500);
    while !shell.readers.iter().all(std::thread::JoinHandle::is_finished) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    shell.unread().whole()
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::process::Stdio;

    use super::*;

    fn started(script: &str, client: u64) -> String {
        let mut command = std::process::Command::new("/bin/sh");
        command.arg("-c").arg(script).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        super::super::start(&super::super::tests::of_client(client), script, &mut command).expect("start")
    }

    #[test]
    fn a_command_is_waited_for_and_its_output_kept() {
        let (stop, detach) = (AtomicBool::new(false), AtomicBool::new(false));
        let id = started("echo out; echo err >&2; exit 3", 911);
        let waited = wait(&id, Duration::from_secs(120), &stop, &detach);
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
            while pressed.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            pressed.store(true, Ordering::SeqCst);
        });
        assert_eq!(wait(&id, Duration::from_secs(120), &stop, &detach), Waited::Detached, "the stale press is ignored");
        ctrl_b.join().expect("ctrl+b");
        assert_eq!(super::super::list(912), [(id.clone(), "sleep 30".to_owned(), true)]);
        assert!(!super::super::kill(999, &id), "another client cannot stop it");
        assert!(super::super::kill(912, &id));
        assert!(super::super::list(912).is_empty());

        let id = started("echo started; sleep 30", 913);
        let deadline = Instant::now() + Duration::from_secs(120);
        while !super::super::tail(913, &id, 100).is_some_and(|seen| seen.contains("started")) {
            assert!(Instant::now() < deadline, "the command never printed");
            std::thread::sleep(Duration::from_millis(5));
        }
        let waited = wait(&id, Duration::from_millis(300), &stop, &AtomicBool::new(false));
        assert!(matches!(&waited, Waited::TimedOut(output) if output.contains("started")), "{waited:?}");
    }
}
