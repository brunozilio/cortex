use std::path::PathBuf;

use domain::SessionId;
use domain::catalog::Listed;
use domain::trace::Record;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionsRequest {
    List,
    History { session: SessionId, cwd: PathBuf },
    Length { session: SessionId, cwd: PathBuf },
    Rename { session: SessionId, title: String },
    Delete { session: SessionId },
    Seen { session: SessionId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionsReply {
    Listed(Vec<Listed>),
    History(Vec<Record>),
    Length(u64),
    Changed,
}
