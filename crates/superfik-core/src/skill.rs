//! A skill or custom command the agent can load by name (`/name`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
}
