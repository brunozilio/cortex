use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    Brief,
    ModelRequest,
    ModelResponse,
    ModelError,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    RunStart,
    RunEnd,
    PermissionRequest,
    Notification,
    TeammateIdle,
    TaskCreated,
    TaskCompleted,
}

impl HookEvent {
    pub const ALL: [Self; 22] = [
        Self::Brief,
        Self::ModelRequest,
        Self::ModelResponse,
        Self::ModelError,
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::Stop,
        Self::SubagentStart,
        Self::SubagentStop,
        Self::PreCompact,
        Self::PostCompact,
        Self::UserPromptSubmit,
        Self::SessionStart,
        Self::SessionEnd,
        Self::RunStart,
        Self::RunEnd,
        Self::PermissionRequest,
        Self::Notification,
        Self::TeammateIdle,
        Self::TaskCreated,
        Self::TaskCompleted,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Brief => "Brief",
            Self::ModelRequest => "ModelRequest",
            Self::ModelResponse => "ModelResponse",
            Self::ModelError => "ModelError",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::Stop => "Stop",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::RunStart => "RunStart",
            Self::RunEnd => "RunEnd",
            Self::PermissionRequest => "PermissionRequest",
            Self::Notification => "Notification",
            Self::TeammateIdle => "TeammateIdle",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == name)
    }

    pub fn may_ask_a_model(self) -> bool {
        !matches!(
            self,
            Self::Brief
                | Self::SessionStart
                | Self::SessionEnd
                | Self::RunStart
                | Self::RunEnd
                | Self::SubagentStart
                | Self::PreCompact
                | Self::PostCompact
                | Self::ModelRequest
                | Self::ModelResponse
                | Self::ModelError
        )
    }

    pub fn takes_plain_text_as_context(self) -> bool {
        matches!(self, Self::SessionStart | Self::RunStart | Self::UserPromptSubmit)
    }

    pub fn fails_quietly(self) -> bool {
        matches!(self, Self::Brief | Self::ModelRequest | Self::ModelResponse | Self::ModelError)
    }

    pub fn after_a_tool(failed: bool) -> Self {
        if failed { Self::PostToolUseFailure } else { Self::PostToolUse }
    }

    pub fn at_the_end(sub_agent: bool) -> Self {
        if sub_agent { Self::SubagentStop } else { Self::Stop }
    }
}

impl PartialOrd for HookEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HookEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl fmt::Display for HookEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HandlerKind {
    Command,
    Script,
    Http,
    Prompt,
    Agent,
}

impl HandlerKind {
    pub const ALL: [Self; 5] = [Self::Command, Self::Script, Self::Http, Self::Prompt, Self::Agent];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Script => "script",
            Self::Http => "http",
            Self::Prompt => "prompt",
            Self::Agent => "agent",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }

    pub fn asks_a_model(self) -> bool {
        matches!(self, Self::Prompt | Self::Agent)
    }
}

impl fmt::Display for HandlerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{HandlerKind, HookEvent};

    #[test]
    fn every_event_is_written_and_read_as_its_own_name() {
        for event in HookEvent::ALL {
            let written = serde_json::to_value(event).expect("written");
            assert_eq!(written, serde_json::json!(event.as_str()));
            assert_eq!(serde_json::from_value::<HookEvent>(written).expect("read"), event);
            assert_eq!(HookEvent::named(event.as_str()), Some(event));
        }
        assert_eq!(HookEvent::named("Stopp"), None);
    }

    #[test]
    fn events_sort_by_name_as_the_settings_file_keys_did() {
        let mut sorted = HookEvent::ALL;
        sorted.sort();
        let mut names = HookEvent::ALL.map(HookEvent::as_str);
        names.sort_unstable();
        assert_eq!(sorted.map(HookEvent::as_str), names);
    }

    #[test]
    fn only_tool_and_turn_events_may_ask_a_model() {
        let asking: Vec<&str> =
            HookEvent::ALL.into_iter().filter(|event| event.may_ask_a_model()).map(HookEvent::as_str).collect();
        assert_eq!(
            asking,
            [
                "PreToolUse",
                "PostToolUse",
                "PostToolUseFailure",
                "Stop",
                "SubagentStop",
                "UserPromptSubmit",
                "PermissionRequest",
                "Notification",
                "TeammateIdle",
                "TaskCreated",
                "TaskCompleted"
            ]
        );
        assert_eq!(HookEvent::named("TeammateIdle"), Some(HookEvent::TeammateIdle));
    }

    #[test]
    fn a_handler_kind_is_written_in_lower_case() {
        for kind in HandlerKind::ALL {
            assert_eq!(serde_json::to_value(kind).expect("written"), serde_json::json!(kind.as_str()));
            assert_eq!(HandlerKind::named(kind.as_str()), Some(kind));
        }
        assert!(HandlerKind::Prompt.asks_a_model() && HandlerKind::Agent.asks_a_model());
        assert!(!HandlerKind::Command.asks_a_model());
        assert_eq!(HandlerKind::named("shell"), None);
    }
}
