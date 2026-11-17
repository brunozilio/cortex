#![allow(clippy::expect_used, reason = "a failed setup step stops the test")]

mod accounts;
mod config;
mod doctor;
mod eval;
mod mcp;
mod plugin;
mod print;
mod surface;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Ran {
    fn from(output: Output) -> Self {
        Self {
            code: output.status.code(),
            stdout: String::from_utf8(output.stdout).expect("stdout is text"),
            stderr: String::from_utf8(output.stderr).expect("stderr is text"),
        }
    }

    pub fn succeeded(&self) -> &Self {
        assert_eq!(self.code, Some(0), "{self:?}");
        self
    }

    pub fn failed_with(&self, code: i32) -> &Self {
        assert_eq!(self.code, Some(code), "{self:?}");
        self
    }

    pub fn said(&self, text: &str) -> &Self {
        assert!(self.stdout.contains(text), "stdout lacks {text:?}: {self:?}");
        self
    }

    pub fn complained(&self, text: &str) -> &Self {
        assert!(self.stderr.contains(text), "stderr lacks {text:?}: {self:?}");
        self
    }

    pub fn quiet_on_stdout(&self) -> &Self {
        assert!(self.stdout.is_empty(), "stdout should be empty: {self:?}");
        self
    }

    pub fn quiet_on_stderr(&self) -> &Self {
        assert!(self.stderr.is_empty(), "stderr should be empty: {self:?}");
        self
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.stdout.trim())
            .unwrap_or_else(|error| panic!("not one JSON value ({error}): {self:?}"))
    }
}

impl std::fmt::Debug for Ran {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit {:?}\n--- stdout\n{}\n--- stderr\n{}", self.code, self.stdout, self.stderr)
    }
}

#[derive(Debug)]
pub struct Sandbox {
    root: tempfile::TempDir,
}

impl Sandbox {
    pub fn fresh() -> Self {
        let root = tempfile::tempdir().expect("temp root");
        for folder in ["home", "project", "tmp"] {
            std::fs::create_dir(root.path().join(folder)).expect("folder");
        }
        Self { root }
    }

    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.root.path()).expect("canonical root")
    }

    pub fn home(&self) -> PathBuf {
        self.root().join("home")
    }

    pub fn project(&self) -> PathBuf {
        self.root().join("project")
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root().join(relative)
    }

    pub fn user_dir(&self) -> PathBuf {
        self.home().join(brand::DIR)
    }

    pub fn project_dir(&self) -> PathBuf {
        self.project().join(brand::DIR)
    }

    pub fn write(&self, path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("parent folder");
        std::fs::write(path, text).expect("written");
    }

    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(binary());
        command
            .args(args)
            .current_dir(self.project())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.home())
            .env("TMPDIR", self.path("tmp"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(brand::env("SECRETS"), "file")
            .stdin(Stdio::null());
        command
    }

    pub fn run(&self, args: &[&str]) -> Ran {
        Ran::from(self.command(args).output().expect("the binary ran"))
    }

    pub fn run_with_input(&self, args: &[&str], input: &str) -> Ran {
        let mut child = self
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the binary started");
        let _ = child.stdin.take().expect("stdin").write_all(input.as_bytes());
        Ran::from(child.wait_with_output().expect("the binary ended"))
    }
}

pub fn command() -> &'static str {
    brand::COMMAND
}

fn binary() -> PathBuf {
    let tests = std::env::current_exe().expect("the test binary");
    let profile = tests.parent().and_then(Path::parent).expect("the profile folder");
    profile.join(format!("{}{}", brand::COMMAND, std::env::consts::EXE_SUFFIX))
}
