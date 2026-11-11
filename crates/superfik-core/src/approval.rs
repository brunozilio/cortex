//! What a running agent asks the user: permission to use a tool, or answers
//! to multiple-choice questions. The run pauses until a [`Decision`] arrives.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AskId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAsk {
    pub id: AskId,
    pub kind: AskKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AskKind {
    Permission {
        tool: String,
        subject: String,
        rememberable: bool,
        /// The allow rule [`Decision::AllowAlways`] keeps in the project's
        /// `settings.local.json` (`Bash(cargo test:*)`); without one it
        /// lasts the session.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule: Option<String>,
    },
    Questions(Vec<Question>),
    /// The agent's plan, put to the user before it leaves plan mode:
    /// [`Decision::Approve`] names the mode to implement it in, `Deny`
    /// keeps planning.
    Plan {
        plan: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub header: String,
    pub text: String,
    pub options: Vec<Choice>,
    pub multi: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    AllowAlways,
    Deny,
    /// Denied, with what the user wants the agent to do instead.
    DenyWith(String),
    Answer(Vec<Vec<usize>>),
    /// A plan approved, to be carried out in this mode.
    Approve(PermissionMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PermissionMode {
    Ask,
    #[default]
    AcceptEdits,
    Plan,
    Bypass,
}

impl PermissionMode {
    pub const ALL: [PermissionMode; 4] = [Self::Ask, Self::AcceptEdits, Self::Plan, Self::Bypass];

    /// The next mode in the order a chat cycles them: ask → edits → plan →
    /// bypass → ask.
    pub const fn next(self) -> Self {
        match self {
            Self::Ask => Self::AcceptEdits,
            Self::AcceptEdits => Self::Plan,
            Self::Plan => Self::Bypass,
            Self::Bypass => Self::Ask,
        }
    }

    const fn latitude(self) -> u8 {
        match self {
            Self::Plan => 0,
            Self::Ask => 1,
            Self::AcceptEdits => 2,
            Self::Bypass => 3,
        }
    }

    /// This mode, or `ceiling` when this one allows more than it.
    pub fn capped(self, ceiling: Option<PermissionMode>) -> PermissionMode {
        match ceiling {
            Some(max) if self.latitude() > max.latitude() => max,
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ceiling_caps_only_modes_that_allow_more() {
        use PermissionMode::*;
        assert_eq!(Bypass.capped(Some(AcceptEdits)), AcceptEdits);
        assert_eq!(Ask.capped(Some(AcceptEdits)), Ask);
        assert_eq!(AcceptEdits.capped(Some(Plan)), Plan);
        assert_eq!(Bypass.capped(None), Bypass);
    }

    #[test]
    fn edits_is_the_default_mode() {
        assert_eq!(PermissionMode::default(), PermissionMode::AcceptEdits);
    }
}
