//! `swap.sh` itself, run with `sh` over temporary folders.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::SWAP_SCRIPT;

fn gone_pid() -> u32 {
    let mut child = Command::new("true").spawn().expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    pid
}

fn install(dir: &Path, content: &str) {
    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::write(dir.join("version"), content).expect("write");
}

fn run_swap(dir: &Path, current: &Path, staged: &Path, pid: u32) -> std::process::ExitStatus {
    run_swap_from(&dir.join("swap.sh"), dir, current, staged, pid)
}

fn run_swap_from(script: &Path, dir: &Path, current: &Path, staged: &Path, pid: u32) -> std::process::ExitStatus {
    std::fs::write(script, SWAP_SCRIPT).expect("script");
    Command::new("/bin/sh")
        .arg(script)
        .arg(pid.to_string())
        .arg(current)
        .arg(staged)
        .arg("touch")
        .arg(dir.join("relaunched"))
        .status()
        .expect("sh")
}

fn wait_for(path: &Path) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if path.exists() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn swaps_the_staged_install_in_and_relaunches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let current = dir.path().join("Superfik.app");
    let staged = dir.path().join(".superfik-update/Superfik.app");
    install(&current, "old");
    install(&staged, "new");
    assert!(run_swap(dir.path(), &current, &staged, gone_pid()).success());
    assert_eq!(std::fs::read_to_string(current.join("version")).ok().as_deref(), Some("new"));
    assert!(!dir.path().join(".superfik-update").exists());
    assert!(!dir.path().join("Superfik.app.superfik-old").exists());
    assert!(!dir.path().join("swap.sh").exists(), "the script removes itself");
    assert!(wait_for(&dir.path().join("relaunched")));
}

#[test]
fn the_script_removes_its_own_folder_too() {
    let dir = tempfile::tempdir().expect("temp dir");
    let current = dir.path().join("Superfik.app");
    let staged = dir.path().join(".superfik-update/Superfik.app");
    install(&current, "old");
    install(&staged, "new");
    let own = dir.path().join("superfik-swap-x");
    std::fs::create_dir(&own).expect("mkdir");
    assert!(run_swap_from(&own.join("swap.sh"), dir.path(), &current, &staged, gone_pid()).success());
    assert!(!own.exists());
    assert!(dir.path().exists(), "a folder still holding things stays");
}

#[test]
fn a_failed_swap_puts_the_old_install_back_and_still_relaunches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let current = dir.path().join("Superfik.app");
    let staged = dir.path().join(".superfik-update/Superfik.app");
    install(&current, "old");
    assert!(run_swap(dir.path(), &current, &staged, gone_pid()).success());
    assert_eq!(std::fs::read_to_string(current.join("version")).ok().as_deref(), Some("old"));
    assert!(!dir.path().join("Superfik.app.superfik-old").exists());
    assert!(wait_for(&dir.path().join("relaunched")));
}

#[test]
fn waits_for_the_app_to_quit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let current = dir.path().join("Superfik.app");
    let staged = dir.path().join(".superfik-update/Superfik.app");
    install(&current, "old");
    install(&staged, "new");
    let mut app = Command::new("sleep").arg("1").spawn().expect("spawn");
    let pid = app.id();
    let reaper = std::thread::spawn(move || app.wait());
    let started = Instant::now();
    assert!(run_swap(dir.path(), &current, &staged, pid).success());
    assert!(started.elapsed() >= Duration::from_millis(500));
    reaper.join().expect("join").expect("wait");
    assert_eq!(std::fs::read_to_string(current.join("version")).ok().as_deref(), Some("new"));
}
