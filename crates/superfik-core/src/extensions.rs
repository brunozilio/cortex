//! What the host program adds to a run (the SDK's): folders whose skills,
//! commands and agents keep their own names, and the run's private MCP
//! config file. Set by the backend, never by a client.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extensions {
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
    #[serde(default)]
    pub mcp_config: Option<PathBuf>,
    /// Superfik plugins' folders (ADR 0019: `.superfik-plugin/plugin.json`,
    /// `skills/`, `agents/`, `commands/`, `hooks/hooks.json`, `.mcp.json`).
    #[serde(default)]
    pub plugins: Vec<PathBuf>,
    /// The owner's home folder for the run's user settings, skills and MCP
    /// servers; the process's when unset.
    #[serde(default)]
    pub home: Option<PathBuf>,
}

impl Extensions {
    pub fn is_empty(&self) -> bool {
        self.dirs.is_empty() && self.mcp_config.is_none() && self.plugins.is_empty() && self.home.is_none()
    }
}
