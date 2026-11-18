use std::process::Stdio;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::tools::Tool;

pub(super) fn of_client(client: u64) -> Workspace {
    let mut workspace = Workspace::new(std::env::temp_dir());
    workspace.credentials.client = client;
    workspace
}

fn workspace(client: u64) -> Workspace {
    of_client(client)
}

#[test]
fn a_started_command_is_remembered_by_the_agent_that_started_it() {
    let starter = of_client(906);
    let id = start(&starter, "quick", &mut sh("true")).expect("start");
    assert_eq!(starter.tracking.started_shells(), std::slice::from_ref(&id));
    assert!(starter.fork(Arc::default()).tracking.started_shells().is_empty());
    assert!(kill(906, &id));
}

#[test]
fn stopping_a_command_this_agent_started_needs_no_permission_and_any_other_asks() {
    use crate::tools::Effect;
    let id = start(&of_client(908), "server", &mut sh("sleep 30")).expect("start");
    let acts =
        |id: &str, client| KillShell.effects(&json!({ "shell_id": id }), &workspace(client)).iter().any(Effect::acts);
    assert!(!acts(&id, 908), "its own command");
    assert!(acts(&id, 909), "another agent's command");
    assert!(acts("bash_nope", 908), "a command it never started");
    assert!(kill(908, &id));
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
    let id = start(&of_client(901), "server", &mut sh("echo ready; sleep 30")).expect("start");
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut first = read(&id, 901);
    while !first.contains("ready") {
        assert!(Instant::now() < deadline, "{first}");
        std::thread::sleep(Duration::from_millis(20));
        first = read(&id, 901);
    }
    assert!(first.contains("running for"), "{first}");
    let unread = BashOutput.run(&json!({ "bash_id": id }), &workspace(901));
    assert!(unread.content.contains("(no new output)"), "output is read once");
    assert_eq!(unread.background, None, "a running command has not ended");
    assert!(read(&id, 902).contains("No background command"), "another client cannot see it");
    assert_eq!(list(901), [(id.clone(), "server".to_owned(), true)]);
    let stopped = KillShell.run(&json!({ "shell_id": id }), &workspace(901));
    assert!(stopped.content.starts_with("Stopped"), "{stopped:?}");
    assert_eq!(stopped.background.as_deref(), Some(&domain::events::BackgroundChange::Ended(id.clone())));
    assert!(list(901).is_empty());
}

#[test]
fn a_person_looks_at_the_last_output_without_taking_it_from_the_agent() {
    let id = start(&of_client(907), "server", &mut sh("echo one; echo two; sleep 30")).expect("start");
    let deadline = Instant::now() + Duration::from_secs(120);
    while !tail(907, &id, 100).is_some_and(|seen| seen.contains("two")) {
        assert!(Instant::now() < deadline, "the output never came");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(tail(907, &id, 4).as_deref(), Some("two"), "only the last bytes, trimmed");
    assert_eq!(tail(908, &id, 100), None, "another client cannot see it");
    assert!(read(&id, 907).contains("one\ntwo"), "the agent still reads it all");
    assert!(kill(907, &id));
}

#[test]
fn a_finished_command_says_how_and_a_leaving_client_stops_its_own() {
    let done = start(&of_client(903), "build", &mut sh("echo built; exit 2")).expect("start");
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut text = read(&done, 903);
    while !text.contains("finished") {
        assert!(Instant::now() < deadline, "{text}");
        std::thread::sleep(Duration::from_millis(20));
        text.push_str(&read(&done, 903));
    }
    assert!(text.contains("exit code 2") && text.contains("built"), "{text}");
    start(&of_client(904), "watch", &mut sh("sleep 30")).expect("start");
    kill_client(904);
    assert!(list(904).is_empty());
}

#[test]
fn stdout_and_stderr_keep_the_order_they_were_printed_in() {
    let (stop, detach) = (std::sync::atomic::AtomicBool::new(false), std::sync::atomic::AtomicBool::new(false));
    let id = start(&of_client(905), "both", &mut sh("echo one; echo two >&2; echo three")).expect("start");
    let Waited::Exited { output, .. } = wait(&id, Duration::from_secs(120), &stop, &detach) else { panic!("ended") };
    assert_eq!(output, "one\ntwo\nthree");
}

#[path = "lifecycle_tests.rs"]
mod lifecycle;
