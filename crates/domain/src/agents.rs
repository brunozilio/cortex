//! Sub-agents a run hands work to, and the agents a project defines.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentStatus {
    Running { progress: f32 },
    Done { seconds: u32 },
    Failed,
}

/// An agent defined in a project, the user's home, or by the host program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomAgent {
    pub name: String,
    pub description: String,
}
