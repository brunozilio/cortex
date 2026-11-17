use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MCP_PREFIX: &str = "mcp__";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolName {
    Advisor,
    Agent,
    AskUserQuestion,
    Bash,
    BashOutput,
    CronCreate,
    CronDelete,
    CronList,
    Edit,
    EnterPlanMode,
    EnterWorktree,
    ExitPlanMode,
    ExitWorktree,
    Glob,
    Grep,
    KillShell,
    Lsp,
    ListMcpResources,
    ListSessions,
    Monitor,
    NotebookEdit,
    PushNotification,
    Repl,
    Read,
    ReadMcpResource,
    ScheduleWakeup,
    SendMessage,
    Skill,
    StructuredOutput,
    TaskCreate,
    TaskGet,
    TaskList,
    TaskOutput,
    TaskStop,
    TaskUpdate,
    ToolSearch,
    Watch,
    WebFetch,
    WebSearch,
    Write,
    Mcp(String),
    Other(String),
}

impl ToolName {
    pub const BUILTIN: [Self; 40] = [
        Self::Advisor,
        Self::Agent,
        Self::AskUserQuestion,
        Self::Bash,
        Self::BashOutput,
        Self::CronCreate,
        Self::CronDelete,
        Self::CronList,
        Self::Edit,
        Self::EnterPlanMode,
        Self::EnterWorktree,
        Self::ExitPlanMode,
        Self::ExitWorktree,
        Self::Glob,
        Self::Grep,
        Self::KillShell,
        Self::Lsp,
        Self::ListMcpResources,
        Self::ListSessions,
        Self::Monitor,
        Self::NotebookEdit,
        Self::PushNotification,
        Self::Repl,
        Self::Read,
        Self::ReadMcpResource,
        Self::ScheduleWakeup,
        Self::SendMessage,
        Self::Skill,
        Self::StructuredOutput,
        Self::TaskCreate,
        Self::TaskGet,
        Self::TaskList,
        Self::TaskOutput,
        Self::TaskStop,
        Self::TaskUpdate,
        Self::ToolSearch,
        Self::Watch,
        Self::WebFetch,
        Self::WebSearch,
        Self::Write,
    ];

    pub fn of(name: &str) -> Self {
        if let Some(builtin) = Self::BUILTIN.into_iter().find(|builtin| builtin.as_str() == name) {
            return builtin;
        }
        if name.starts_with(MCP_PREFIX) { Self::Mcp(name.to_owned()) } else { Self::Other(name.to_owned()) }
    }

    pub const fn as_str(&self) -> &str {
        match self {
            Self::Advisor => "Advisor",
            Self::Agent => "Agent",
            Self::AskUserQuestion => "AskUserQuestion",
            Self::Bash => "Bash",
            Self::BashOutput => "BashOutput",
            Self::CronCreate => "CronCreate",
            Self::CronDelete => "CronDelete",
            Self::CronList => "CronList",
            Self::Edit => "Edit",
            Self::EnterPlanMode => "EnterPlanMode",
            Self::EnterWorktree => "EnterWorktree",
            Self::ExitPlanMode => "ExitPlanMode",
            Self::ExitWorktree => "ExitWorktree",
            Self::Glob => "Glob",
            Self::Grep => "Grep",
            Self::KillShell => "KillShell",
            Self::Lsp => "LSP",
            Self::ListMcpResources => "ListMcpResources",
            Self::ListSessions => "ListSessions",
            Self::Monitor => "Monitor",
            Self::NotebookEdit => "NotebookEdit",
            Self::PushNotification => "PushNotification",
            Self::Repl => "REPL",
            Self::Read => "Read",
            Self::ReadMcpResource => "ReadMcpResource",
            Self::ScheduleWakeup => "ScheduleWakeup",
            Self::SendMessage => "SendMessage",
            Self::Skill => "Skill",
            Self::StructuredOutput => "StructuredOutput",
            Self::TaskCreate => "TaskCreate",
            Self::TaskGet => "TaskGet",
            Self::TaskList => "TaskList",
            Self::TaskOutput => "TaskOutput",
            Self::TaskStop => "TaskStop",
            Self::TaskUpdate => "TaskUpdate",
            Self::ToolSearch => "ToolSearch",
            Self::Watch => "Watch",
            Self::WebFetch => "WebFetch",
            Self::WebSearch => "WebSearch",
            Self::Write => "Write",
            Self::Mcp(name) | Self::Other(name) => name.as_str(),
        }
    }

    pub fn is_builtin(&self) -> bool {
        !matches!(self, Self::Mcp(_) | Self::Other(_))
    }

    pub fn mcp_server(&self) -> Option<&str> {
        match self {
            Self::Mcp(name) => name.strip_prefix(MCP_PREFIX)?.split("__").next(),
            _ => None,
        }
    }

    pub fn mcp_tool(&self) -> Option<&str> {
        match self {
            Self::Mcp(name) => name.strip_prefix(MCP_PREFIX)?.split_once("__").map(|(_, tool)| tool),
            _ => None,
        }
    }

    pub fn plans_tasks(&self) -> bool {
        matches!(self, Self::TaskCreate | Self::TaskGet | Self::TaskList | Self::TaskUpdate)
    }

    pub fn is_a_task_tool(&self) -> bool {
        self.plans_tasks() || matches!(self, Self::TaskOutput | Self::TaskStop)
    }

    pub fn changes_the_task_list(&self) -> bool {
        matches!(self, Self::TaskCreate | Self::TaskUpdate)
    }

    pub fn reads_files(&self) -> bool {
        matches!(self, Self::Read | Self::Glob | Self::Grep)
    }

    pub fn reads_mcp_resources(&self) -> bool {
        matches!(self, Self::ReadMcpResource | Self::ListMcpResources)
    }

    pub fn reads_the_web(&self) -> bool {
        matches!(self, Self::WebFetch | Self::WebSearch)
    }
}

impl From<&str> for ToolName {
    fn from(name: &str) -> Self {
        Self::of(name)
    }
}

impl PartialEq<str> for ToolName {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for ToolName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

impl Serialize for ToolName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ToolName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|name| Self::of(&name))
    }
}

#[cfg(test)]
mod tests {
    use super::ToolName;

    #[test]
    fn every_builtin_name_reads_back_as_itself_and_anything_else_stays_as_written() {
        for builtin in ToolName::BUILTIN {
            assert_eq!(ToolName::of(builtin.as_str()), builtin);
            assert!(builtin.is_builtin());
            let written = serde_json::to_value(&builtin).expect("written");
            assert_eq!(written, serde_json::json!(builtin.as_str()));
            assert_eq!(serde_json::from_value::<ToolName>(written).expect("read"), builtin);
        }
        assert_eq!(ToolName::of("LSP"), ToolName::Lsp);
        assert_eq!(ToolName::of("REPL"), ToolName::Repl);
        assert_eq!(ToolName::of("deploy"), ToolName::Other("deploy".into()));
        assert_eq!(ToolName::of("mcp__docs__read"), ToolName::Mcp("mcp__docs__read".into()));
        assert_eq!(ToolName::of("mcp__docs__read").as_str(), "mcp__docs__read");
    }

    #[test]
    fn an_mcp_tool_names_its_server_and_its_own_name() {
        let tool = ToolName::of("mcp__docs__read__all");
        assert_eq!((tool.mcp_server(), tool.mcp_tool()), (Some("docs"), Some("read__all")));
        assert_eq!(ToolName::Bash.mcp_server(), None);
    }

    #[test]
    fn task_tools_that_plan_are_told_from_the_ones_that_collect_background_work() {
        assert!(ToolName::TaskCreate.plans_tasks() && ToolName::TaskList.plans_tasks());
        assert!(!ToolName::TaskOutput.plans_tasks() && ToolName::TaskOutput.is_a_task_tool());
        assert!(ToolName::TaskUpdate.changes_the_task_list() && !ToolName::TaskList.changes_the_task_list());
        assert!(!ToolName::Read.is_a_task_tool());
    }
}
