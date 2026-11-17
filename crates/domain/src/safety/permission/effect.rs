use std::path::PathBuf;

use crate::shell::ShellCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub written: PathBuf,
    pub real: PathBuf,
}

impl Place {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self { written: path.clone(), real: path }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnKind {
    Agent,
    BackgroundShell,
    Schedule,
    Watch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declared {
    Edits,
    Executes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exec {
    pub command: ShellCommand,
    pub unsandboxed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    ReadPath(Place),
    ReadTree(Place),
    WritePath(Place),
    Exec(Exec),
    Net { host: Option<String> },
    Spawn(SpawnKind),
    Cancel { own: bool },
    Notify,
    Session { to: String },
    Resource { server: String, uri: String },
    Remote { server: String },
    Declared(Declared),
    Install { server: String, what: String },
}

impl Effect {
    pub fn exec(command: &str, unsandboxed: bool) -> Self {
        Self::Exec(Exec { command: ShellCommand::parse(command), unsandboxed })
    }

    pub fn acts(&self) -> bool {
        match self {
            Self::ReadPath(_)
            | Self::ReadTree(_)
            | Self::WritePath(_)
            | Self::Notify
            | Self::Cancel { own: true }
            | Self::Spawn(SpawnKind::Agent | SpawnKind::Watch)
            | Self::Declared(Declared::Edits) => false,
            Self::Exec(_)
            | Self::Net { .. }
            | Self::Spawn(SpawnKind::BackgroundShell | SpawnKind::Schedule)
            | Self::Cancel { own: false }
            | Self::Session { .. }
            | Self::Resource { .. }
            | Self::Remote { .. }
            | Self::Install { .. }
            | Self::Declared(Declared::Executes) => true,
        }
    }

    pub fn changes_files(&self) -> bool {
        matches!(self, Self::WritePath(_) | Self::Declared(Declared::Edits))
    }
}

pub fn change_nothing(effects: &[Effect]) -> bool {
    !effects.iter().any(|effect| effect.acts() || effect.changes_files())
}
