use crate::transcript::{Block, Message, Role};

#[derive(Debug, Default)]
pub struct Reminders {
    sent: Option<String>,
}

impl Reminders {
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
    pub(super) fn runtime_reminder(&self, used: u64, threshold: u64) -> String {
        let context = self.workspace.prompt_context.as_ref();
        let style = context.and_then(|context| context.output_style.as_ref());
        let reminder =
            crate::prompt::reminder(self.control.mode(), self.depth, style.and_then(|s| s.reminder.as_deref()));
        let today = crate::engine::date::today();
        let offers_search = self.request.tools.iter().any(|spec| spec.name == crate::tools::tool_search::TOOL_SEARCH);
        let deferred =
            self.workspace.deferred.as_ref().filter(|_| offers_search).map(|deferred| deferred.announcement());
        let notes: String =
            [deferred, context.and_then(|context| date_changed(&context.date, &today)), context_left(used, threshold)]
                .into_iter()
                .flatten()
                .map(|note| format!("\n\n{note}"))
                .collect();
        format!("{reminder}{}{notes}", self.workspace.rule_instructions())
    }
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
