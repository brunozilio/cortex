//! Chromium lets one process own a cache root: it leaves a `SingletonLock`
//! link there naming `<host>-<pid>`, and a second process starting on the
//! same root hands its launch over to the first, which opens a window of
//! its own (a "New Tab", like opening Chrome twice). A second Superfik
//! therefore runs without pages instead of starting CEF.

use std::path::Path;

pub fn held_elsewhere(cache_root: &Path) -> bool {
    let Ok(target) = std::fs::read_link(cache_root.join("SingletonLock")) else { return false };
    let Some(pid) = target.to_str().and_then(lock_pid) else { return false };
    pid != std::process::id() && alive(pid)
}

fn lock_pid(target: &str) -> Option<u32> {
    target.rsplit_once('-')?.1.parse().ok()
}

fn alive(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_names_its_process_after_the_last_dash() {
        assert_eq!(lock_pid("Brunos-MacBook-Pro.local-32631"), Some(32631));
        assert_eq!(lock_pid("nopid"), None);
    }

    #[cfg(unix)]
    #[test]
    fn only_another_live_process_holds_the_root() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        assert!(!held_elsewhere(dir.path()), "no lock");
        let lock = dir.path().join("SingletonLock");
        std::os::unix::fs::symlink(format!("host-{}", std::process::id()), &lock)?;
        assert!(!held_elsewhere(dir.path()), "our own lock");
        std::fs::remove_file(&lock)?;
        let mut other = std::process::Command::new("/bin/sleep").arg("5").spawn()?;
        std::os::unix::fs::symlink(format!("host-{}", other.id()), &lock)?;
        let held = held_elsewhere(dir.path());
        other.kill()?;
        other.wait()?;
        assert!(held, "a live process");
        assert!(!held_elsewhere(dir.path()), "a dead one leaves a stale lock");
        Ok(())
    }
}
