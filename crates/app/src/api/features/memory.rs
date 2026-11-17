use std::path::PathBuf;

use domain::memory::seed::Candidate;
use domain::memory::{Forgotten, Memory, MemoryEdit, MemoryId};
use domain::{ModelOption, RunId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRequest {
    pub path: PathBuf,
    pub op: MemoryOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryOp {
    List,
    Search { query: String },
    Edit { edits: Vec<MemoryEdit> },
    Undo,
    Share { id: MemoryId },
    Check,
    Score,
    Seed,
    KeepSeeds { picked: Vec<usize> },
    Optimize { run: RunId, model: ModelOption },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryListing {
    pub path: PathBuf,
    pub memories: Vec<Memory>,
    pub forgotten: Vec<Forgotten>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryReply {
    Listing(MemoryListing),
    Found { path: PathBuf, query: String, scores: Vec<(MemoryId, f32)> },
    Shared { path: PathBuf, file: PathBuf },
    Check(MemoryCheck),
    Score { path: PathBuf, report: String },
    Seeds { path: PathBuf, candidates: Vec<Candidate> },
    Optimizing { run: RunId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryCheck {
    pub path: PathBuf,
    pub prompt: Option<String>,
    pub changed: usize,
    pub memories: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryApplied {
    pub optimizing: bool,
    pub added: usize,
    pub changed: usize,
    pub forgotten: usize,
    pub listing: MemoryListing,
}
