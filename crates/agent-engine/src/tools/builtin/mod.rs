pub mod advisor;
pub mod code_review;
pub mod files;
pub mod follow_through;
pub mod notify;
pub mod schedules;
pub mod shell;
pub mod verification;
pub mod web;

use crate::agent::Review;
use crate::tools::Tool;

pub fn tools() -> Vec<Box<dyn Tool>> {
    [advisor::tools(), files::tools(), notify::tools(), schedules::tools(), shell::tools(), web::tools()]
        .into_iter()
        .flatten()
        .collect()
}

pub fn reviewers() -> Vec<Box<dyn Review>> {
    [code_review::review::reviewers(), follow_through::review::reviewers(), verification::review::reviewers()]
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tools: Vec<Box<dyn Tool>>) -> Vec<String> {
        tools.iter().map(|tool| tool.spec().name).collect()
    }

    #[test]
    fn each_tool_folder_registers_these_tools_in_this_order() {
        let folders = [
            ("advisor", names(advisor::tools())),
            ("files", names(files::tools())),
            ("notify", names(notify::tools())),
            ("schedules", names(schedules::tools())),
            ("shell", names(shell::tools())),
            ("web", names(web::tools())),
        ];
        let listed: Vec<String> =
            folders.iter().map(|(folder, tools)| format!("{folder}: {}", tools.join(" "))).collect();
        assert_eq!(
            listed,
            [
                "advisor: Advisor",
                "files: Read Write Edit Glob Grep NotebookEdit",
                "notify: PushNotification",
                "schedules: CronCreate CronList CronDelete ScheduleWakeup",
                "shell: Bash BashOutput KillShell Monitor",
                "web: WebFetch WebSearch",
            ]
        );
        assert_eq!(names(tools()), folders.into_iter().flat_map(|(_, tools)| tools).collect::<Vec<String>>());
    }

    #[test]
    fn the_reviewers_come_from_these_folders_in_this_order() {
        let folders = [
            ("code_review", code_review::review::reviewers().len()),
            ("follow_through", follow_through::review::reviewers().len()),
            ("verification", verification::review::reviewers().len()),
        ];
        assert_eq!(folders, [("code_review", 1), ("follow_through", 1), ("verification", 1)]);
        assert_eq!(reviewers().len(), 3);
    }
}
