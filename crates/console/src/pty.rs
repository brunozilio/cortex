use std::io::{Read, Write};
use std::path::PathBuf;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde_json::Value;

const DEFAULT: Size = Size { cols: 80, rows: 24 };

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

impl Launch {
    pub fn this_program(cwd: PathBuf) -> Self {
        let program = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(brand::COMMAND));
        Self { program, args: Vec::new(), cwd }
    }

    pub fn resuming(&self, session: &str, folder: Option<PathBuf>) -> Self {
        let mut args = self.args.clone();
        args.extend(["--resume".to_owned(), session.to_owned()]);
        Self { program: self.program.clone(), args, cwd: folder.unwrap_or_else(|| self.cwd.clone()) }
    }
}

pub fn session_id(text: &str) -> Option<&str> {
    let allowed = |char: char| char.is_ascii_alphanumeric() || char == '-' || char == '_';
    Some(text).filter(|text| !text.is_empty() && !text.starts_with('-') && text.chars().all(allowed))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
}

impl Size {
    pub fn asked(cols: Option<&String>, rows: Option<&String>) -> Self {
        let read =
            |value: Option<&String>| value.and_then(|value| value.parse::<u16>().ok()).filter(|value| *value > 0);
        Self { cols: read(cols).unwrap_or(DEFAULT.cols), rows: read(rows).unwrap_or(DEFAULT.rows) }
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let read =
            |field: &str| value[field].as_u64().and_then(|value| u16::try_from(value).ok()).filter(|value| *value > 0);
        Some(Self { cols: read("cols")?, rows: read("rows")? })
    }

    fn pty(self) -> PtySize {
        PtySize { rows: self.rows, cols: self.cols, pixel_width: 0, pixel_height: 0 }
    }
}

pub struct Opened {
    pub master: Box<dyn MasterPty + Send>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Owned,
}

pub struct Owned(Option<Box<dyn Child + Send + Sync>>);

impl Drop for Owned {
    fn drop(&mut self) {
        let Some(mut child) = self.0.take() else { return };
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        let _ = child.kill();
        let _ = std::thread::Builder::new().name("terminal-reaper".into()).spawn(move || child.wait());
    }
}

pub fn open(launch: &Launch, size: Size) -> Result<Opened, String> {
    let pair = native_pty_system().openpty(size.pty()).map_err(|error| error.to_string())?;
    let mut command = CommandBuilder::new(&launch.program);
    command.args(&launch.args);
    command.cwd(&launch.cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    let child = pair.slave.spawn_command(command).map_err(|error| error.to_string())?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader().map_err(|error| error.to_string())?;
    let writer = pair.master.take_writer().map_err(|error| error.to_string())?;
    Ok(Opened { master: pair.master, reader, writer, child: Owned(Some(child)) })
}

pub fn resize(master: &dyn MasterPty, size: Size) {
    let _ = master.resize(size.pty());
}

#[cfg(all(test, unix))]
#[allow(clippy::expect_used, reason = "a failed setup step stops the test")]
mod tests {
    use super::*;

    #[test]
    fn letting_go_of_a_terminal_whose_program_ignores_the_hangup_does_not_wait_for_it() {
        let launch = Launch {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "trap '' HUP TERM; sleep 30".into()],
            cwd: std::env::temp_dir(),
        };
        let opened = open(&launch, Size { cols: 80, rows: 24 }).expect("opened");
        std::thread::sleep(std::time::Duration::from_millis(200));
        let started = std::time::Instant::now();
        drop(opened);
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
    }
}
