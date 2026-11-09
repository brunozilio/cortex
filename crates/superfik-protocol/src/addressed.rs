//! Who a server event is for. A local backend has one client and
//! delivers everything to it; a shared server fans events out by these.

use serde::{Deserialize, Serialize};
use superfik_core::{Principal, RunId};

use crate::ServerEvent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recipient {
    Principal(Principal),
    Run(RunId),
    Everyone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Addressed {
    pub to: Recipient,
    pub event: ServerEvent,
}

impl ServerEvent {
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error { message: message.into() }
    }

    pub fn is_run_terminal(&self) -> bool {
        matches!(self, Self::Run { event, .. } if event.is_terminal())
    }
}
