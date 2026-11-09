//! Routines: runs started without a human prompt (ADR 0012).
//!
//! A routine belongs to its project, private to its owner, like notes. It
//! fires on its `Schedule` (or at once, when asked to run now); each firing
//! is one run whose outcome is kept as a `Report` for the operations inbox.
//! Its `Autonomy` decides how far the run may act alone. Time is passed in,
//! so every rule here is pure.

mod schedule;

use serde::{Deserialize, Serialize};

use crate::{Connection, PermissionMode, RunId};

pub use prompt::Cause;
pub use schedule::*;
pub use template::Template;
pub use webhook::{MAX_WEBHOOK_BYTES, WebhookId, WebhookLimit, WebhookToken};

mod prompt;
mod template;
pub mod webhook;

pub const MAX_BRIEF_CHARS: usize = 8_000;
pub const MAX_REPORT_CHARS: usize = 16_000;
pub const MAX_REPORTS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RoutineId(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Autonomy {
    #[default]
    Draft,
    Bounded,
    Auto,
}

impl Autonomy {
    pub const ALL: [Autonomy; 3] = [Self::Draft, Self::Bounded, Self::Auto];

    pub const fn mode(self) -> PermissionMode {
        match self {
            Self::Draft => PermissionMode::Plan,
            Self::Bounded => PermissionMode::AcceptEdits,
            Self::Auto => PermissionMode::Bypass,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: RoutineId,
    pub name: String,
    pub brief: String,
    pub schedule: Schedule,
    #[serde(default)]
    pub autonomy: Autonomy,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub answers: bool,
    #[serde(default)]
    pub webhook: Option<WebhookId>,
    pub enabled: bool,
    #[serde(default)]
    pub next_run: Option<i64>,
    #[serde(default)]
    pub last_run: Option<i64>,
}

impl Routine {
    pub fn works_apart(&self) -> bool {
        self.autonomy != Autonomy::Draft
    }

    pub fn branch(&self, unix: i64) -> String {
        let (year, month, day) = crate::days::civil(unix.div_euclid(86_400));
        let minute = unix.rem_euclid(86_400) / 60;
        let ascii: String = self.name.chars().filter(char::is_ascii).collect();
        let name = crate::note::slug(&ascii);
        let name = if name.is_empty() { self.id.0.to_string() } else { name };
        format!("routine/{name}-{year:04}{month:02}{day:02}-{:02}{:02}", minute / 60, minute % 60)
    }

    pub fn listens_to(&self, connection: crate::ConnectionId) -> bool {
        self.enabled && self.answers && self.connections.iter().any(|c| c.id == connection)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub channel: String,
    pub exchange: String,
    pub from: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Ending {
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub routine: RoutineId,
    pub run: RunId,
    pub started: i64,
    #[serde(default)]
    pub finished: Option<i64>,
    #[serde(default)]
    pub ending: Option<Ending>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
}

impl Report {
    pub fn signal(&self) -> Option<crate::job::JobSignal> {
        crate::job::JobSignal::find(&self.text)
    }

    pub fn is_running(&self) -> bool {
        self.ending.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineDraft {
    pub name: String,
    pub brief: String,
    pub schedule: Schedule,
    #[serde(default)]
    pub autonomy: Autonomy,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub answers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RoutineEdit {
    Create { draft: RoutineDraft },
    Revise { id: RoutineId, draft: RoutineDraft },
    Enable { id: RoutineId, enabled: bool },
    RunNow { id: RoutineId },
    Webhook { id: RoutineId, on: bool },
    Delete { id: RoutineId },
    Read { run: RunId },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub created: Vec<RoutineId>,
    pub changed: Vec<RoutineId>,
    pub refused: Vec<RoutineId>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoutineBook {
    #[serde(default)]
    pub next: u32,
    #[serde(default)]
    pub routines: Vec<Routine>,
    #[serde(default)]
    pub reports: Vec<Report>,
}

mod book;
mod policy;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_answers;
#[cfg(test)]
mod tests_webhook;
