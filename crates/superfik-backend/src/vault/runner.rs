//! Runs the command an agent asked to run with secrets in its environment.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
}

pub trait CommandRunner: Send + Sync {
    fn run(&self, command: &str, cwd: &Path, env: &[(String, String)], limit: Duration) -> Result<Ran, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ShellRunner;

const POLL: Duration = Duration::from_millis(25);

impl CommandRunner for ShellRunner {
    fn run(&self, command: &str, cwd: &Path, env: &[(String, String)], limit: Duration) -> Result<Ran, String> {
        let mut tree = superfik_process::Tree::spawn(
            shell(command)
                .current_dir(cwd)
                .envs(env.iter().map(|(k, v)| (k, v)))
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )
        .map_err(|error| error.to_string())?;
        let drain = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                if let Some(mut pipe) = pipe {
                    let _ = pipe.read_to_end(&mut bytes);
                }
                bytes
            })
        };
        let stdout = drain(tree.child().stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let stderr = drain(tree.child().stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let started = Instant::now();
        let (status, timed_out) = loop {
            if let Some(status) = tree.child().try_wait().map_err(|error| error.to_string())? {
                break (Some(status), false);
            }
            if started.elapsed() >= limit {
                tree.kill();
                break (tree.child().wait().ok(), true);
            }
            std::thread::sleep(POLL);
        };
        let mut output = stdout.join().unwrap_or_default();
        output.extend(stderr.join().unwrap_or_default());
        Ok(Ran {
            code: status.and_then(|s| s.code()).filter(|_| !timed_out),
            output: String::from_utf8_lossy(&output).into_owned(),
            timed_out,
        })
    }
}

#[cfg(unix)]
fn shell(command: &str) -> Command {
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
    let mut cmd = Command::new(shell);
    cmd.args(["-lc", command]);
    cmd
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    superfik_process::WindowsShell::detect().login_command(command)
}

#[cfg(all(test, unix))]
mod time_limit_tests {
    use super::*;

    #[test]
    fn the_time_limit_ends_what_the_command_started() -> Result<(), String> {
        let started = Instant::now();
        let ran = ShellRunner.run("sleep 30 & sleep 30", Path::new("/"), &[], Duration::from_millis(200))?;
        assert!(ran.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
        Ok(())
    }
}
