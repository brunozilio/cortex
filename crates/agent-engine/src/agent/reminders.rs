use crate::transcript::{Block, Message, Role};

#[derive(Debug, Default)]
pub struct Reminders {
    sent: Option<String>,
    quiet_tasks: usize,
}

impl Reminders {
    pub fn observe(&mut self, calls: &[(String, String, serde_json::Value)]) {
        if calls.iter().any(|(_, name, _)| name.starts_with("Task") && name != "TaskOutput" && name != "TaskStop") {
            self.quiet_tasks = 0;
        } else if !calls.is_empty() {
            self.quiet_tasks += 1;
        }
    }

    pub fn quiet_tasks(&self) -> usize {
        self.quiet_tasks
    }

    pub fn deliver(&mut self, current: String, history: &mut Vec<Message>) {
        if self.sent.as_ref() == Some(&current) || current.trim().is_empty() {
            return;
        }
        let block = Block::Reminder(current.trim().to_owned());
        match history.last_mut() {
            Some(last) if last.role == Role::User => last.blocks.push(block),
            _ => history.push(Message { role: Role::User, blocks: vec![block] }),
        }
        self.sent = Some(current);
    }

    pub fn resend(&mut self) {
        self.sent = None;
    }
}

impl super::Agent<'_> {
    pub(super) fn runtime_reminder(&self, used: u64, threshold: u64, quiet_tasks: usize) -> String {
        let context = self.workspace.prompt_context.as_ref();
        let style = context.and_then(|context| context.output_style.as_ref());
        let reminder =
            crate::prompt::reminder(self.control.mode(), self.depth, style.and_then(|s| s.reminder.as_deref()));
        let today = crate::engine::date::today();
        let offers_search = self.request.tools.iter().any(|spec| spec.name == crate::tools::tool_search::TOOL_SEARCH);
        let deferred =
            self.workspace.deferred.as_ref().filter(|_| offers_search).map(|deferred| deferred.announcement());
        let tasks = if quiet_tasks >= QUIET_TURNS { self.open_tasks() } else { Vec::new() };
        let notes: String = [
            deferred,
            context.and_then(|context| date_changed(&context.date, &today)),
            context_left(used, threshold),
            tasks_left_alone(quiet_tasks, &tasks),
        ]
        .into_iter()
        .flatten()
        .map(|note| format!("\n\n{note}"))
        .collect();
        format!("{reminder}{}{notes}", self.workspace.rule_instructions())
    }
}

pub const QUIET_TURNS: usize = 10;

pub fn tasks_left_alone(quiet_turns: usize, open: &[String]) -> Option<String> {
    if quiet_turns < QUIET_TURNS || open.is_empty() {
        return None;
    }
    Some(format!(
        "The task tools have not been used for a while. If the list below is stale, update it: mark finished tasks \
         completed and drop the ones that no longer apply. Do not mention this reminder to the user.\n{}",
        open.iter().map(|task| format!("- {task}")).collect::<Vec<_>>().join("\n")
    ))
}

pub fn date_changed(session_date: &str, today: &str) -> Option<String> {
    (!session_date.is_empty() && session_date != today)
        .then(|| format!("The date has changed. Today's date is now {today}. No need to announce it."))
}

pub fn context_left(used: u64, threshold: u64) -> Option<String> {
    if threshold == 0 {
        return None;
    }
    let left = threshold.saturating_sub(used) * 100 / threshold;
    let shown = match left {
        0..=10 => 10,
        11..=25 => 25,
        _ => return None,
    };
    Some(format!(
        "Less than {shown}% of the context is left before older messages are summarized. Finish the current step \
         and keep tool output small."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_with_tools_but_no_task_tool_count_as_quiet() {
        let call = |name: &str| vec![("1".to_owned(), name.to_owned(), serde_json::Value::Null)];
        let mut reminders = Reminders::default();
        reminders.observe(&call("Read"));
        reminders.observe(&call("TaskOutput"));
        reminders.observe(&[]);
        assert_eq!(reminders.quiet_tasks(), 2, "a final answer is not a quiet turn");
        reminders.observe(&call("TaskUpdate"));
        assert_eq!(reminders.quiet_tasks(), 0);
    }

    #[test]
    fn open_tasks_are_recalled_only_after_the_task_tools_went_quiet() {
        let open = vec!["Write the test (in_progress)".to_owned()];
        assert_eq!(tasks_left_alone(QUIET_TURNS - 1, &open), None);
        assert_eq!(tasks_left_alone(QUIET_TURNS, &[]), None, "nothing open, nothing to say");
        let note = tasks_left_alone(QUIET_TURNS, &open).expect("note");
        assert!(note.contains("- Write the test (in_progress)"));
        assert_eq!(tasks_left_alone(QUIET_TURNS + 5, &open), Some(note), "the same text, so it is sent once");
    }

    #[test]
    fn a_new_day_is_told_once_it_differs_from_the_session_s() {
        assert_eq!(date_changed("2026-11-11", "2026-11-11"), None);
        assert!(date_changed("2026-11-11", "2026-11-12").is_some_and(|text| text.contains("now 2026-11-12")));
        assert_eq!(date_changed("", "2026-11-12"), None);
    }

    #[test]
    fn the_context_left_is_told_in_steps_so_the_reminder_stays_the_same() {
        assert_eq!(context_left(10, 100), None);
        assert_eq!(context_left(76, 100), context_left(80, 100));
        assert!(context_left(80, 100).is_some_and(|text| text.contains("25%")));
        assert!(context_left(95, 100).is_some_and(|text| text.contains("10%")));
        assert_eq!(context_left(5, 0), None);
    }

    #[test]
    fn a_reminder_rides_on_the_latest_user_message_and_only_when_it_changes() {
        let mut reminders = Reminders::default();
        let mut history = vec![Message::user("fix it")];
        reminders.deliver("mode: ask".into(), &mut history);
        reminders.deliver("mode: ask".into(), &mut history);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text(), "fix it", "the user's own words stay apart");
        assert_eq!(history[0].blocks[1], Block::Reminder("mode: ask".into()));

        history.push(Message::assistant(vec![]));
        reminders.deliver("mode: plan".into(), &mut history);
        assert_eq!(history.len(), 3, "a new user message carries a change");
        reminders.resend();
        reminders.deliver("mode: plan".into(), &mut history);
        assert_eq!(history[2].blocks.len(), 2, "sent again after compaction dropped it");
    }
}
