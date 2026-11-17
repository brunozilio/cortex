use std::path::PathBuf;

use domain::features::Listed;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum FlagsRequest {
    List { cwd: PathBuf },
    Set { cwd: PathBuf, name: String, on: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedFlag {
    pub flag: Listed,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagsReply {
    pub flags: Vec<ListedFlag>,
}
