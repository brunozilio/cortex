use domain::output::Saving;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSavingsReply {
    pub savings: Vec<Saving>,
}
