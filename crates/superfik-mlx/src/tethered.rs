//! A process that dies with the app. macOS has no "kill me when my parent
//! dies", so the server runs under `sh`, which also reads a pipe held by
//! the app: when the pipe closes (the app stopped it, quit or crashed), the
//! server is killed. A model server left behind would hold tens of
//! gigabytes.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::process::{Child, ChildStdin, Stdio};

const SCRIPT: &str = r#"exec 3<&0; "$@" </dev/null & pid=$!; (read _ <&3; kill "$pid" 2>/dev/null) & wait "$pid""#;

pub(super) struct Tethered {
    child: Child,
    tether: Option<ChildStdin>,
}

impl Tethered {
    pub(super) fn spawn(program: &OsStr, args: &[&OsStr], log: File) -> io::Result<Self> {
        let mut child = superfik_process::command("/bin/sh")
            .args(["-c", SCRIPT, "sh"])
            .arg(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()?;
        let tether = child.stdin.take();
        Ok(Self { child, tether })
    }

    pub(super) fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub(super) fn stop(&mut self) {
        self.tether.take();
        let _ = self.child.wait();
    }
}

impl Drop for Tethered {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::*;

    fn sleeper(dir: &Path) -> (Tethered, std::path::PathBuf) {
        let pidfile = dir.join("pid");
        let script = format!("echo $$ > '{}'; exec sleep 30", pidfile.display());
        let log = File::create(dir.join("log")).expect("log");
        let tethered =
            Tethered::spawn(OsStr::new("/bin/sh"), &[OsStr::new("-c"), OsStr::new(&script)], log).expect("spawn");
        (tethered, pidfile)
    }

    fn pid(pidfile: &Path) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(pid) = std::fs::read_to_string(pidfile).map(|p| p.trim().to_owned())
                && !pid.is_empty()
            {
                return pid;
            }
            assert!(Instant::now() < deadline, "the server never started");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn alive(pid: &str) -> bool {
        std::process::Command::new("kill").args(["-0", pid]).status().is_ok_and(|s| s.success())
    }

    fn gone_soon(pid: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(pid) {
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }

    #[test]
    fn the_server_dies_when_the_app_lets_go_of_the_pipe() {
        let dir = tempfile::tempdir().expect("dir");
        let (mut tethered, pidfile) = sleeper(dir.path());
        let server = pid(&pidfile);
        assert!(alive(&server) && tethered.is_running());
        tethered.tether.take();
        assert!(gone_soon(&server));
        tethered.stop();
    }

    #[test]
    fn a_server_that_exits_is_no_longer_running() {
        let dir = tempfile::tempdir().expect("dir");
        let (mut tethered, pidfile) = sleeper(dir.path());
        let server = pid(&pidfile);
        std::process::Command::new("kill").arg(&server).status().expect("kill");
        let deadline = Instant::now() + Duration::from_secs(5);
        while tethered.is_running() {
            assert!(Instant::now() < deadline, "still running");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
