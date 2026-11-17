use std::path::PathBuf;

use domain::changes::{Changes, DiffScope};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRequest {
    pub path: PathBuf,
    pub op: WorkspaceOp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceOp {
    Changes { scope: DiffScope },
    Files,
    GitOffer,
    GitInit,
    Mentions { prompt: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceReply {
    Changes(Changes),
    Files(Vec<String>),
    GitOffer { offer: bool },
    GitInit { committed: bool },
    Mentions(String),
}
