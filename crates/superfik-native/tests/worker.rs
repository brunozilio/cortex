//! Real subprocess boundary. No Claude/Codex binary or live provider involved.
use serde_json::json;
use std::path::Path;
use superfik_native::tools::{self, Tool, Workspace};
fn remote(name: &str) -> Box<dyn Tool> {
    superfik_native::worker::wrap(tools::standard(), Some(Path::new(env!("CARGO_BIN_EXE_superfik-native-worker"))))
        .into_iter()
        .find(|t| t.spec().name == name)
        .expect("valid test fixture")
}
#[test]
fn worker_preserves_read_before_edit_and_returns_real_output() {
    let dir = tempfile::tempdir().expect("valid test fixture");
    std::fs::write(dir.path().join("file"), "before").expect("valid test fixture");
    let workspace = Workspace::new(dir.path());
    let edit = json!({"file_path":"file","old_string":"before","new_string":"after"});
    assert!(remote("Edit").run(&edit, &workspace).is_error);
    assert!(!remote("Read").run(&json!({"file_path":"file"}), &workspace).is_error);
    let changed = remote("Edit").run(&edit, &workspace);
    assert!(!changed.is_error, "{}", changed.content);
    assert_eq!(std::fs::read_to_string(dir.path().join("file")).expect("valid test fixture"), "after");
}
#[cfg(unix)]
#[test]
fn worker_cancellation_stops_its_shell_before_returning() {
    use std::{
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };
    let dir = tempfile::tempdir().expect("valid test fixture");
    let workspace = Arc::new(Workspace::new(dir.path()));
    let task_workspace = workspace.clone();
    let task = std::thread::spawn(move || {
        remote("Bash").run(&json!({"command":"echo started > started; sleep 1; echo leaked > leaked"}), &task_workspace)
    });
    for _ in 0..100 {
        if dir.path().join("started").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(dir.path().join("started").exists());
    workspace.stop.store(true, Ordering::Relaxed);
    let outcome = task.join().expect("valid test fixture");
    assert!(outcome.is_error);
    std::thread::sleep(Duration::from_millis(1100));
    assert!(!dir.path().join("leaked").exists());
}
#[test]
fn worker_shell_does_not_inherit_provider_or_github_host_keys() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut worker = Command::new(env!("CARGO_BIN_EXE_superfik-native-worker"))
        .env("ANTHROPIC_API_KEY", "host-key")
        .env("OPENAI_API_KEY", "host-key")
        .env("GH_TOKEN", "host-identity")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("valid test fixture");
    let mut input = worker.stdin.take().expect("valid test fixture");
    writeln!(input,"{}",json!({"name":"Bash","cwd":std::env::temp_dir(),"input":{"command":"printf '%s:%s:%s' \"${ANTHROPIC_API_KEY-unset}\" \"${OPENAI_API_KEY-unset}\" \"${GH_TOKEN-unset}\""},"github":null,"read":false})).expect("valid test fixture");
    let output = worker.wait_with_output().expect("valid test fixture");
    let result: tools::Outcome = serde_json::from_slice(&output.stdout).expect("valid test fixture");
    assert!(!result.is_error, "{}", result.content);
    assert_eq!(result.content, "unset:unset:unset");
}
