//! The system tools an install calls (`ditto`, `tar`, `codesign`) and the
//! detached swap, behind `Commands`.

use std::ffi::OsString;
use std::process::{Command, Stdio};

use crate::UpdateError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub success: bool,
    pub output: String,
}

pub trait Commands: Send + Sync {
    fn run(&self, program: &str, args: &[OsString]) -> Result<Ran, UpdateError>;
    fn detach(&self, program: &str, args: &[OsString]) -> Result<(), UpdateError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct System;

impl Commands for System {
    fn run(&self, program: &str, args: &[OsString]) -> Result<Ran, UpdateError> {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| UpdateError::Install(format!("{program}: {error}")))?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(Ran { success: output.status.success(), output: text })
    }

    fn detach(&self, program: &str, args: &[OsString]) -> Result<(), UpdateError> {
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        command.spawn().map(drop).map_err(|error| UpdateError::Install(format!("{program}: {error}")))
    }
}
