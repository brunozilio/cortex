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
    Permission { tool: String, subject: String, rememberable: bool },
    Questions(Vec<Question>),
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
    Answer(Vec<Vec<usize>>),
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

    pub const fn cli_value(self) -> &'static str {
        match self {
            Self::Ask => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Plan => "plan",
            Self::Bypass => "bypassPermissions",
        }
    }

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

impl Question {
    pub fn answer_text(&self, indices: &[usize]) -> String {
        indices.iter().filter_map(|i| self.options.get(*i)).map(|c| c.label.as_str()).collect::<Vec<_>>().join(", ")
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

    fn question(multi: bool) -> Question {
        let choice = |label: &str| Choice { label: label.into(), description: String::new() };
        Question { header: "File".into(), text: "Which?".into(), options: vec![choice("a"), choice("b")], multi }
    }

    #[test]
    fn answer_text_joins_picked_labels_in_order() {
        assert_eq!(question(false).answer_text(&[1]), "b");
        assert_eq!(question(true).answer_text(&[0, 1]), "a, b");
        assert_eq!(question(true).answer_text(&[7]), "", "out of range is ignored");
    }

    #[test]
    fn modes_map_to_cli_values() {
        let values: Vec<_> = PermissionMode::ALL.iter().map(|m| m.cli_value()).collect();
        assert_eq!(values, ["default", "acceptEdits", "plan", "bypassPermissions"]);
        assert_eq!(PermissionMode::default(), PermissionMode::AcceptEdits);
    }
}
