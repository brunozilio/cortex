//! The system tools an install calls (`ditto`, `tar`, `codesign`,
//! PowerShell) and the detached swap, behind `Commands`.

use std::ffi::OsString;
use std::process::{Command, Stdio};

use crate::UpdateError;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

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
        let mut command = Command::new(program);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
        let output = command
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
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP,
        );
        command.spawn().map(drop).map_err(|error| UpdateError::Install(format!("{program}: {error}")))
    }
}
