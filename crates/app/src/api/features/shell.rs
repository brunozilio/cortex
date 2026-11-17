use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ShellRequest {
    List,
    Kill { id: String },
    Run { command: String, path: PathBuf },
    Output { id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundCommand {
    pub id: String,
    pub command: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellReply {
    Listed(Vec<BackgroundCommand>),
    Ran { output: String, failed: bool },
    Output(String),
}
