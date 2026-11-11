use std::path::Path;

use agent_engine::tools::{Workspace, subagent_config::AgentConfig};
use domain::{OrgId, Principal, UserId};
use serde_json::json;

fn git(cwd: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git");
    assert!(status.success());
}

#[test]
fn memory_persists_across_worker_invocations_and_is_separated_by_owner() {
    let dir = tempfile::tempdir().expect("dir");
    let mut workspace = Workspace::new(dir.path());
    workspace.credentials.owner = Some(Principal::member(UserId::new(), OrgId::new()));
    let config = AgentConfig::parse(&json!({"memory":"project","tools":["Read"]})).expect("config");
    assert!(config.allows("Write") && config.allows("Edit"));
    let first = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("first setup");
    let memory = first.memory_dir.expect("memory dir").join("MEMORY.md");
    std::fs::write(memory, "Persistent project fact").expect("write memory");
    let second = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("second setup");
    assert_eq!(second.memory, "Persistent project fact");
    workspace.credentials.owner = Some(Principal::member(UserId::new(), OrgId::new()));
    let other = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("other owner");
    assert!(other.memory.is_empty());
    assert_ne!(second.memory_dir, other.memory_dir);
}
#[test]
fn isolated_worktrees_remove_clean_runs_and_preserve_changed_files() {
    let dir = tempfile::tempdir().expect("dir");
    git(dir.path(), &["init", "-b", "main"]);
    std::fs::write(dir.path().join("base.txt"), "base").expect("base");
    git(dir.path(), &["add", "base.txt"]);
    git(dir.path(), &["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
    let workspace = Workspace::new(dir.path());
    let config = AgentConfig::parse(&json!({"isolation":"worktree"})).expect("config");
    let clean = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("clean setup");
    assert_ne!(clean.cwd, workspace.cwd);
    assert!(clean.cwd.join("base.txt").exists());
    assert!(agent_engine::agent_runtime::cleanup(&clean).expect("clean remove").is_none());
    assert!(!clean.cwd.exists());
    let changed = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("dirty setup");
    std::fs::write(changed.cwd.join("new.txt"), "worktree change").expect("write");
    assert_eq!(agent_engine::agent_runtime::cleanup(&changed).expect("preserve"), Some(changed.cwd.clone()));
    assert!(!workspace.cwd.join("new.txt").exists());
    assert!(changed.cwd.join("new.txt").exists());
}

#[test]
fn worktree_with_new_commits_is_retained_even_when_status_is_clean() {
    let dir = tempfile::tempdir().expect("dir");
    git(dir.path(), &["init", "-b", "main"]);
    std::fs::write(dir.path().join("base.txt"), "base").expect("base");
    git(dir.path(), &["add", "base.txt"]);
    git(dir.path(), &["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
    let workspace = Workspace::new(dir.path());
    let config = AgentConfig::parse(&json!({"isolation":"worktree"})).expect("config");
    let setup = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("setup");
    git(
        &setup.cwd,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "agent commit",
        ],
    );
    assert_eq!(agent_engine::agent_runtime::cleanup(&setup).expect("preserve commits"), Some(setup.cwd.clone()));
    assert!(setup.cwd.exists());
}

#[test]
#[cfg(unix)]
fn read_rejects_a_fifo_without_waiting_for_a_writer() {
    use agent_engine::tools::Tool;
    let dir = tempfile::tempdir().expect("dir");
    let fifo = dir.path().join("pipe");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().expect("mkfifo").success());
    let workspace = Workspace::new(dir.path());
    let start = std::time::Instant::now();
    let result = agent_engine::tools::Read.run(&json!({"file_path":"pipe"}), &workspace);
    assert!(result.is_error && result.content.contains("regular file"));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}
#[test]
#[cfg(unix)]
fn cancellation_during_worktree_creation_removes_the_registered_clean_checkout() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    git(dir.path(), &["init", "-b", "main"]);
    std::fs::write(dir.path().join("base.txt"), "base").expect("base");
    git(dir.path(), &["add", "base.txt"]);
    git(dir.path(), &["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
    let marker = dir.path().join("hook-ready");
    let quoted = format!("'{}'", marker.to_string_lossy().replace('\'', "'\\''"));
    let hook = dir.path().join(".git/hooks/post-checkout");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch {quoted}\nsleep 20\n")).expect("hook");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    let workspace = Workspace::new(dir.path());
    let stop = workspace.stop.clone();
    let thread = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let config = AgentConfig::parse(&json!({"isolation":"worktree"})).expect("config");
    assert!(agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).is_err());
    thread.join().expect("cancel");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("worktrees");
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().filter(|line| line.starts_with("worktree ")).count(), 1);
}
