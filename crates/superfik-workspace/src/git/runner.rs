//! Running `git` commands, behind a trait so parsing can be tested.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::WorkspaceError;

pub trait GitRunner: Send + Sync {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError>;

    fn run_with(&self, cwd: &Path, args: &[&str], _env: &[(String, String)]) -> Result<String, WorkspaceError> {
        self.run(cwd, args)
    }
}

#[derive(Debug, Clone)]
pub struct SystemGit {
    program: PathBuf,
}

impl SystemGit {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into() }
    }
}

impl Default for SystemGit {
    fn default() -> Self {
        let system = Path::new("/usr/bin/git");
        if system.is_file() {
            return Self::new(system);
        }
        Self::new(crate::executable::locate("git", "SUPERFIK_GIT").unwrap_or_else(|| PathBuf::from("git")))
    }
}

pub(crate) fn global_args(os: &str) -> &'static [&'static str] {
    if os == "windows" { &["-c", "core.longpaths=true"] } else { &[] }
}

impl GitRunner for SystemGit {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
        self.run_with(cwd, args, &[])
    }

    fn run_with(&self, cwd: &Path, args: &[&str], env: &[(String, String)]) -> Result<String, WorkspaceError> {
        let output = superfik_process::command(&self.program)
            .args(global_args(std::env::consts::OS))
            .arg("-C")
            .arg(cwd)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .envs(env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .output()
            .map_err(|source| WorkspaceError::Io { path: self.program.clone(), source })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(WorkspaceError::Git {
                command: args.join(" "),
                message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_git_takes_long_paths() {
        assert_eq!(global_args("windows"), ["-c", "core.longpaths=true"]);
        assert!(global_args("macos").is_empty());
    }
}
