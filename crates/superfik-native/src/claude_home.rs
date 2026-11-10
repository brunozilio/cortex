//! The layout of a Claude Code home directory (`~/.claude`), as found on
//! real installs:
//!
//! - `skills/<name>/SKILL.md` and `agents/<name>.md`: the user's own.
//! - `skills/synced/<bucket>/<name>/SKILL.md`: skills synced from
//!   claude.ai; Claude Code exposes them as `anthropic-skills:<name>`.
//!
//! Its plugins are not read: Superfik's are its own (`crate::plugins`).

use std::fs;
use std::path::{Path, PathBuf};

pub const SYNCED_NAMESPACE: &str = "anthropic-skills";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub dir: PathBuf,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeHome {
    root: PathBuf,
}

impl ClaudeHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn detect() -> Option<Self> {
        std::env::home_dir().filter(|home| !home.as_os_str().is_empty()).map(|home| Self::new(home.join(".claude")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The user's own and synced skills.
    pub fn skill_sources(&self) -> Vec<Source> {
        let mut sources = vec![Source { dir: self.root.join("skills"), namespace: None }];
        sources.extend(
            subdirs(&self.root.join("skills").join("synced"))
                .into_iter()
                .map(|dir| Source { dir, namespace: Some(SYNCED_NAMESPACE.to_owned()) }),
        );
        sources
    }

    pub fn agent_sources(&self) -> Vec<Source> {
        vec![Source { dir: self.root.join("agents"), namespace: None }]
    }
}

pub(crate) fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    dirs.sort();
    dirs
}
