use domain::events::TaskItem;

use super::app::{App, Msg};

pub fn skills(app: &App) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.skills(&cwd) {
        Ok(skills) if skills.is_empty() => Msg::Notice("No skills or commands found.".into()),
        Ok(skills) => {
            let lines: Vec<String> = skills.iter().map(|s| format!("  /{:<20} {}", s.name, s.description)).collect();
            Msg::Notice(format!("Skills and commands:\n{}", lines.join("\n")))
        }
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn agents(app: &App) {
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.agents(&cwd) {
        Ok(agents) => {
            let lines: Vec<String> = agents.iter().map(|a| format!("  {:<20} {}", a.name, a.description)).collect();
            Msg::Notice(format!("Agents:\n{}", lines.join("\n")))
        }
        Err(error) => Msg::Error(error.to_string()),
    });
}

pub fn todos(app: &mut App) {
    let text = tasks(&app.transcript.tasks);
    app.transcript.notice(text);
}

fn tasks(tasks: &[TaskItem]) -> String {
    if tasks.is_empty() {
        return "No tasks yet: the agent makes a list for work with several steps.".to_owned();
    }
    let lines: Vec<String> = tasks
        .iter()
        .map(|task| {
            let mark = match task.status.as_str() {
                "completed" => "☒",
                "in_progress" => "◼",
                _ => "☐",
            };
            format!("  {mark} {}", task.subject)
        })
        .collect();
    format!("Tasks:\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(subject: &str, status: &str) -> TaskItem {
        TaskItem { id: subject.into(), subject: subject.into(), status: status.into() }
    }

    #[test]
    fn tasks_are_marked_by_their_status() {
        let listed = tasks(&[task("plan", "completed"), task("build", "in_progress"), task("ship", "pending")]);
        assert_eq!(listed, "Tasks:\n  ☒ plan\n  ◼ build\n  ☐ ship");
        assert!(tasks(&[]).starts_with("No tasks yet"));
    }
}
