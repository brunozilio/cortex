use std::path::Path;

use agent_engine::tools::{Workspace, subagent_config::AgentConfig};
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
fn memory_persists_across_worker_invocations() {
    let dir = tempfile::tempdir().expect("dir");
    let workspace = Workspace::new(dir.path());
    let config = AgentConfig::parse(&json!({"memory":"project","tools":["Read"]})).expect("config");
    assert!(config.allows("Write") && config.allows("Edit"));
    let first = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("first setup");
    let memory = first.memory_dir.expect("memory dir").join("MEMORY.md");
    std::fs::write(memory, "Persistent project fact").expect("write memory");
    let second = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect("second setup");
    assert_eq!(second.memory, "Persistent project fact");
    let other = agent_engine::agent_runtime::prepare(&config, "planner", &workspace).expect("another agent");
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
    let dir = tempfile::tempdir().expect("dir");
    let fifo = dir.path().join("pipe");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().expect("mkfifo").success());
    let workspace = Workspace::new(dir.path());
    let start = std::time::Instant::now();
    let result = agent_engine::tools::named("Read").expect("Read").run(&json!({"file_path":"pipe"}), &workspace);
    assert!(result.is_error && result.content.contains("regular file"));
    assert!(start.elapsed() < std::time::Duration::from_secs(60), "{:?}", start.elapsed());
}
const SLOW_CHECKOUT_REPO: &str = "SLOW_CHECKOUT_REPO";

#[test]
#[cfg(unix)]
fn a_child_cancels_a_worktree_while_its_checkout_runs() {
    let Some(repo) = std::env::var_os(SLOW_CHECKOUT_REPO).map(std::path::PathBuf::from) else { return };
    let marker = repo.join("checkout-started");
    let workspace = Workspace::new(&repo);
    let stop = workspace.stop.clone();
    let thread = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let config = AgentConfig::parse(&json!({"isolation":"worktree"})).expect("config");
    let refused = agent_engine::agent_runtime::prepare(&config, "reviewer", &workspace).expect_err("cancelled");
    thread.join().expect("cancel");
    println!("refused: {refused}");
}

#[test]
#[cfg(unix)]
fn cancellation_during_worktree_creation_keeps_the_partial_checkout_and_says_where() {
    let dir = tempfile::tempdir().expect("dir");
    git(dir.path(), &["init", "-b", "main"]);
    std::fs::write(dir.path().join("base.txt"), "base").expect("base");
    std::fs::write(dir.path().join(".gitattributes"), "base.txt filter=slow\n").expect("attributes");
    git(dir.path(), &["add", "base.txt", ".gitattributes"]);
    git(dir.path(), &["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "base"]);
    let marker = dir.path().join("checkout-started");
    let quoted = format!("'{}'", marker.to_string_lossy().replace('\'', "'\\''"));
    let users = tempfile::tempdir().expect("user config");
    let config = users.path().join("gitconfig");
    std::fs::write(&config, format!("[filter \"slow\"]\n\tsmudge = \"touch {quoted}; sleep 20; cat\"\n"))
        .expect("config");

    let child = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "runtime::a_child_cancels_a_worktree_while_its_checkout_runs", "--test-threads=1"])
        .arg("--nocapture")
        .env(SLOW_CHECKOUT_REPO, dir.path())
        .env("GIT_CONFIG_GLOBAL", &config)
        .output()
        .expect("child");

    let said = String::from_utf8_lossy(&child.stdout);
    assert!(child.status.success(), "{said}");
    assert!(marker.exists(), "the user's own smudge filter ran, so the checkout was under way");
    let retained = said
        .lines()
        .find_map(|line| line.split_once("partial checkout retained at ").map(|(_, path)| path.trim().to_owned()))
        .unwrap_or_else(|| panic!("the refusal names the partial checkout: {said}"));
    assert!(said.contains("cancelled or timed out"), "{said}");
    let worktrees = dir.path().canonicalize().expect("repo").join(concat!(brand::dir!(), "/worktrees"));
    assert!(std::path::Path::new(&retained).starts_with(&worktrees), "{retained}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("worktrees");
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().filter(|line| line.starts_with("worktree ")).count(), 2);
}
