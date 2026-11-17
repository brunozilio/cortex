use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginsRequest {
    List,
    Install { source: String, name: Option<String> },
    Update { name: String },
    Remove { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginsReply {
    Listed(Vec<Plugin>),
    Installed { name: String, at: PathBuf },
    Updated { name: String, git: String },
    Removed { name: String },
}
