use std::path::PathBuf;

use domain::permission_rules::RuleKind;
use domain::suggested_rules::Suggestion;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SuggestedRulesRequest {
    List { cwd: PathBuf },
    KeepRule { cwd: PathBuf, kind: RuleKind, rule: String },
    KeepInstruction { cwd: PathBuf, line: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedRulesReply {
    Suggested(Vec<Suggestion>),
    RuleKept,
    InstructionKept { file: PathBuf },
}
