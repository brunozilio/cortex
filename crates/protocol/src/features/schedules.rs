use domain::schedule::{Scheduled, When};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SchedulesRequest {
    List,
    TakeDue,
    Delete { id: String },
    Create { prompt: String, when: When },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulesReply {
    pub due: Vec<Scheduled>,
    pub scheduled: Vec<Scheduled>,
}
