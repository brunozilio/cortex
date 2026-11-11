use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extensions {
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
    #[serde(default)]
    pub mcp_config: Option<PathBuf>,
    #[serde(default)]
    pub plugins: Vec<PathBuf>,
    #[serde(default)]
    pub home: Option<PathBuf>,
}

impl Extensions {
    pub fn is_empty(&self) -> bool {
        self.dirs.is_empty() && self.mcp_config.is_none() && self.plugins.is_empty() && self.home.is_none()
    }
}
