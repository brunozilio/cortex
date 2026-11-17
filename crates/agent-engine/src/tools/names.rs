pub use domain::ToolName;

pub const ASK: &str = ToolName::AskUserQuestion.as_str();
pub const ADVISOR: &str = ToolName::Advisor.as_str();
pub const ENTER_PLAN: &str = ToolName::EnterPlanMode.as_str();
pub const EXIT_PLAN: &str = ToolName::ExitPlanMode.as_str();
pub const REPL: &str = ToolName::Repl.as_str();
pub const SEND_MESSAGE: &str = ToolName::SendMessage.as_str();
pub const SKILL: &str = ToolName::Skill.as_str();
pub const AGENT: &str = ToolName::Agent.as_str();
pub const WATCH: &str = ToolName::Watch.as_str();
pub const ENTER_WORKTREE: &str = ToolName::EnterWorktree.as_str();
pub const EXIT_WORKTREE: &str = ToolName::ExitWorktree.as_str();
pub const BASH: &str = ToolName::Bash.as_str();
pub const READ: &str = ToolName::Read.as_str();
pub const WRITE: &str = ToolName::Write.as_str();
pub const WEB_FETCH: &str = ToolName::WebFetch.as_str();
pub const STRUCTURED_OUTPUT: &str = ToolName::StructuredOutput.as_str();
pub const TASK_OUTPUT: &str = ToolName::TaskOutput.as_str();
pub const TASK_STOP: &str = ToolName::TaskStop.as_str();
pub const TASK_LIST: &str = ToolName::TaskList.as_str();
pub const TASK_GET: &str = ToolName::TaskGet.as_str();
pub const TASK_CREATE: &str = ToolName::TaskCreate.as_str();
pub const TASK_UPDATE: &str = ToolName::TaskUpdate.as_str();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_name_the_loop_dispatches_on_belongs_to_a_standard_tool_or_one_the_run_adds() {
        let mut offered: Vec<String> = crate::tools::standard().iter().map(|tool| tool.spec().name).collect();
        offered.extend(crate::tools::task::tools().iter().map(|tool| tool.spec().name));
        for name in
            [ASK, ENTER_PLAN, EXIT_PLAN, ENTER_WORKTREE, EXIT_WORKTREE, WATCH, BASH, READ, WRITE, WEB_FETCH, ADVISOR]
        {
            assert!(offered.iter().any(|known| known == name), "{name} is not a standard tool");
        }
        for name in [TASK_OUTPUT, TASK_STOP, TASK_LIST, TASK_GET, TASK_CREATE, TASK_UPDATE] {
            assert!(offered.iter().any(|known| known == name), "{name} is not a task tool");
            assert!(ToolName::of(name).is_a_task_tool());
        }
        for name in &offered {
            assert!(ToolName::of(name).is_builtin(), "{name} has no ToolName");
        }
    }
}
