pub use super::ask::ASK_TOOL as ASK;
pub use super::builtin::advisor::ADVISOR_TOOL as ADVISOR;
pub use super::plan_mode::{ENTER_PLAN_TOOL as ENTER_PLAN, EXIT_PLAN_TOOL as EXIT_PLAN};
pub use super::repl::REPL_TOOL as REPL;
pub use super::send_message::SEND_MESSAGE_TOOL as SEND_MESSAGE;
pub use super::skill::SKILL_TOOL as SKILL;
pub use super::subagent::AGENT_TOOL as AGENT;
pub use super::watch::WATCH_TOOL as WATCH;
pub use super::worktree::{ENTER_WORKTREE, EXIT_WORKTREE};

pub const BASH: &str = "Bash";
pub const READ: &str = "Read";
pub const WEB_FETCH: &str = "WebFetch";
pub const STRUCTURED_OUTPUT: &str = "StructuredOutput";
pub const TASK_OUTPUT: &str = "TaskOutput";
pub const TASK_STOP: &str = "TaskStop";
pub const TASK_LIST: &str = "TaskList";
pub const TASK_CREATE: &str = "TaskCreate";
pub const TASK_UPDATE: &str = "TaskUpdate";
pub const TASK_FAMILY: &str = "Task";

pub fn plans_tasks(name: &str) -> bool {
    name.starts_with(TASK_FAMILY) && name != TASK_OUTPUT && name != TASK_STOP
}

pub fn changes_the_task_list(name: &str) -> bool {
    matches!(name, TASK_CREATE | TASK_UPDATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_the_loop_dispatches_on_belongs_to_a_standard_tool_or_one_the_run_adds() {
        let standard: Vec<String> = crate::tools::standard().iter().map(|tool| tool.spec().name).collect();
        for name in [ASK, ENTER_PLAN, EXIT_PLAN, ENTER_WORKTREE, EXIT_WORKTREE, WATCH, BASH, READ, WEB_FETCH, ADVISOR] {
            assert!(standard.iter().any(|known| known == name), "{name} is not a standard tool");
        }
        let tasks: Vec<String> = crate::tools::task::tools().iter().map(|tool| tool.spec().name).collect();
        for name in [TASK_OUTPUT, TASK_STOP, TASK_LIST, TASK_CREATE, TASK_UPDATE] {
            assert!(tasks.iter().any(|known| known == name), "{name} is not a task tool");
            assert!(name.starts_with(TASK_FAMILY));
        }
    }

    #[test]
    fn the_task_tools_that_plan_are_told_from_the_ones_that_collect_background_work() {
        assert!(plans_tasks(TASK_CREATE) && plans_tasks(TASK_UPDATE) && plans_tasks(TASK_LIST));
        assert!(!plans_tasks(TASK_OUTPUT) && !plans_tasks(TASK_STOP) && !plans_tasks(READ));
        assert!(changes_the_task_list(TASK_CREATE) && !changes_the_task_list(TASK_LIST));
    }
}
