use std::process::Stdio;
use std::time::Duration;

use super::*;

fn workspace(client: u64) -> Workspace {
    let mut workspace = Workspace::new(std::env::temp_dir());
    workspace.credentials.client = client;
    workspace
}

fn sh(script: &str) -> std::process::Command {
    let mut command = std::process::Command::new("/bin/sh");
    command.arg("-c").arg(script).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command
}

fn read(id: &str, client: u64) -> String {
    BashOutput.run(&json!({ "bash_id": id }), &workspace(client)).content
}

#[test]
fn a_background_command_is_read_incrementally_and_stopped() {
    let id = start(901, "server", &mut sh("echo ready; sleep 30")).expect("start");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut first = read(&id, 901);
    while !first.contains("ready") {
        assert!(Instant::now() < deadline, "{first}");
        std::thread::sleep(Duration::from_millis(20));
        first = read(&id, 901);
    }
    assert!(first.contains("running for"), "{first}");
    assert!(read(&id, 901).contains("(no new output)"), "output is read once");
    assert!(read(&id, 902).contains("No background command"), "another client cannot see it");
    assert_eq!(list(901), [(id.clone(), "server".to_owned(), true)]);
    let stopped = KillShell.run(&json!({ "shell_id": id }), &workspace(901));
    assert!(stopped.content.starts_with("Stopped"), "{stopped:?}");
    assert!(list(901).is_empty());
}

#[test]
fn a_finished_command_says_how_and_a_leaving_client_stops_its_own() {
    let done = start(903, "build", &mut sh("echo built; exit 2")).expect("start");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut text = read(&done, 903);
    while !text.contains("finished") {
        assert!(Instant::now() < deadline, "{text}");
        std::thread::sleep(Duration::from_millis(20));
        text.push_str(&read(&done, 903));
    }
    assert!(text.contains("exit code 2") && text.contains("built"), "{text}");
    start(904, "watch", &mut sh("sleep 30")).expect("start");
    kill_client(904);
    assert!(list(904).is_empty());
}

#[test]
fn stdout_and_stderr_keep_the_order_they_were_printed_in() {
    let (stop, detach) = (std::sync::atomic::AtomicBool::new(false), std::sync::atomic::AtomicBool::new(false));
    let id = start(905, "both", &mut sh("echo one; echo two >&2; echo three")).expect("start");
    let Waited::Exited { output, .. } = wait(&id, Duration::from_secs(5), &stop, &detach) else { panic!("ended") };
    assert_eq!(output, "one\ntwo\nthree");
}

#[test]
fn unread_output_keeps_only_its_newest_part() {
    let mut kept = vec![b'a'; 2 * KEPT];
    capped(&mut kept);
    assert_eq!(kept.len(), 2 * KEPT, "at the limit nothing goes");
    kept.extend_from_slice(b"newest");
    capped(&mut kept);
    assert_eq!(kept.len(), KEPT);
    assert!(kept.ends_with(b"newest"));
}
