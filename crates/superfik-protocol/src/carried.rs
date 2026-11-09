//! Small shapes some `ServerEvent`s carry.

use serde::{Deserialize, Serialize};
use superfik_core::SessionRecord;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedSession {
    pub owner: superfik_core::UserId,
    pub owner_name: String,
    pub session: SessionRecord,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSettings {
    pub domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberUsage {
    pub user: superfik_core::UserId,
    pub name: String,
    pub month: String,
    pub cost_usd: f64,
    pub runs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewer {
    pub user: superfik_core::UserId,
    pub name: String,
    pub session: superfik_core::ConversationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    Lost,
    Restored,
    SignedOut,
}
