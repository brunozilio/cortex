use std::io::{BufRead, BufReader};
use std::os::unix::process::ExitStatusExt;

use super::*;

fn sh(script: &str) -> Command {
    let mut command = crate::command("/bin/sh");
    command.args(["-c", script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    command
}

fn first_line(child: &Child) -> String {
    let stdout = child.take_stdout().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).expect("line");
    line.trim().to_owned()
}

fn gone_within(pid: u32, wait: Duration) -> bool {
    let until = Instant::now() + wait;
    while crate::alive(pid) && Instant::now() < until {
        std::thread::sleep(POLL);
    }
    !crate::alive(pid)
}

fn end(pid: u32) {
    let _ = crate::terminate(pid);
}

#[test]
fn a_dropped_child_leaves_no_zombie() {
    let child = Child::spawn(&mut sh("sleep 30")).expect("spawn");
    let pid = child.id();
    assert!(child.is_alive());
    let dropping = Instant::now();
    drop(child);
    assert!(dropping.elapsed() < Duration::from_secs(1), "the drop waited {:?}", dropping.elapsed());
    assert!(gone_within(pid, Duration::from_secs(10)), "pid {pid} is still there, running or a zombie");
}

#[test]
fn a_dropped_child_that_ignores_the_polite_stop_is_killed_and_reaped() {
    let child = Child::spawn(&mut sh("trap '' TERM; echo ready; while :; do sleep 0.1; done")).expect("spawn");
    assert_eq!(first_line(&child), "ready");
    let pid = child.id();
    drop(child);
    assert!(crate::alive(pid), "it was given no grace");
    assert!(gone_within(pid, GRACE + Duration::from_secs(10)), "pid {pid} outlived the grace");
}

#[test]
fn what_the_child_started_dies_with_it() {
    let child = Child::spawn(&mut sh("sleep 30 & echo $!; wait")).expect("spawn");
    let grandchild: u32 = first_line(&child).parse().expect("pid");
    assert!(crate::alive(grandchild));
    let status = child.terminate(Duration::from_secs(5));
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    assert!(gone_within(grandchild, Duration::from_secs(5)), "the grandchild outlived the group");
    assert!(!child.is_alive());
}

#[test]
fn a_child_that_ignores_the_polite_stop_is_killed_after_the_grace() {
    let child = Child::spawn(&mut sh("trap '' TERM; echo ready; while :; do sleep 0.1; done")).expect("spawn");
    assert_eq!(first_line(&child), "ready");
    let asked = Instant::now();
    let grace = Duration::from_millis(300);
    let status = child.terminate(grace);
    assert_eq!(status.signal(), Some(libc::SIGKILL));
    assert!(asked.elapsed() >= grace, "killed before the grace was over: {:?}", asked.elapsed());
    assert!(gone_within(child.id(), Duration::from_secs(1)));

    let polite = Child::spawn(&mut sh("exec sleep 30")).expect("spawn");
    let asked = Instant::now();
    assert_eq!(polite.terminate(Duration::from_secs(30)).signal(), Some(libc::SIGTERM));
    assert!(asked.elapsed() < Duration::from_secs(10), "a child that obeys is not waited for");
}

#[test]
fn the_end_of_what_a_child_said_on_stderr_is_kept_after_it_failed_to_start() {
    let tail = Spawn { stderr: Stderr::Tail, ..Spawn::default() };
    let mut failing = sh("echo 'config.json: no such file' >&2; exit 3");
    let child = Child::spawn_with(&mut failing, tail).expect("spawn");
    let status = child.wait_until(Instant::now() + Duration::from_secs(10)).expect("exited");
    assert_eq!(status.code(), Some(3));
    assert_eq!(child.stderr_tail(), "config.json: no such file");

    let mut noisy = sh("head -c 200000 /dev/zero | tr '\\0' 'x' >&2; echo the last line >&2");
    let child = Child::spawn_with(&mut noisy, tail).expect("spawn");
    child.wait_until(Instant::now() + Duration::from_secs(10)).expect("exited");
    let kept = child.stderr_tail();
    assert!(kept.len() <= STDERR_KEPT, "{} bytes kept", kept.len());
    assert!(kept.ends_with("the last line"), "{}", &kept[kept.len().saturating_sub(40)..]);
}

#[test]
fn nothing_is_signalled_once_the_child_has_been_reaped() {
    let kept = Spawn { leftovers: Leftovers::Kept, ..Spawn::default() };
    let child = Child::spawn_with(&mut sh("sleep 30 & echo $!"), kept).expect("spawn");
    let in_its_group: u32 = first_line(&child).parse().expect("pid");
    let status = child.wait_until(Instant::now() + Duration::from_secs(10)).expect("exited");
    assert!(status.success());

    assert_eq!(child.terminate(Duration::from_secs(1)), status);
    assert_eq!(child.kill(), status);
    drop(child);
    std::thread::sleep(Duration::from_millis(100));
    assert!(crate::alive(in_its_group), "the reaped child's group was signalled");
    end(in_its_group);
}

#[test]
fn what_an_exited_child_left_running_ends_before_it_is_reaped() {
    let child = Child::spawn(&mut sh("sleep 30 & echo $!")).expect("spawn");
    let left: u32 = first_line(&child).parse().expect("pid");
    assert!(child.wait_until(Instant::now() + Duration::from_secs(10)).is_some());
    assert!(gone_within(left, Duration::from_secs(5)), "the grandchild outlived its exited parent");
}

#[test]
fn a_child_can_be_shared_between_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<Child>();
}
