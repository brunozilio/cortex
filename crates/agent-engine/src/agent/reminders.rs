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

#[cfg(test)]
mod tests {
    use super::*;

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
