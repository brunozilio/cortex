use std::path::PathBuf;

use domain::SessionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TraceRequest {
    EverySession,
    Session { session: SessionId, cwd: PathBuf },
    Follow { session: SessionId, cwd: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceReply {
    Report(serde_json::Value),
    Followed(serde_json::Value),
}
