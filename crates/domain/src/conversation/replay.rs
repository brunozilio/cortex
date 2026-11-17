use super::{Entry, SessionStatus, ToolState, Transcript};
use crate::EngineEvent;
use crate::trace::Record;

impl Transcript {
    pub fn replay(&mut self, records: &[Record]) {
        self.follow(records);
        self.left_unfinished();
    }

    pub fn follow(&mut self, records: &[Record]) -> usize {
        let (mut shown, mut used) = (None, 0);
        for (at, record) in records.iter().enumerate() {
            match record {
                Record::Shown { text, attached } => {
                    shown = Some((text.clone(), attached.clone()));
                    continue;
                }
                Record::Prompted { prompt, .. } => {
                    self.left_unfinished();
                    let (prompt, names) = shown.take().unwrap_or_else(|| crate::attachment::written(prompt));
                    if !names.is_empty() {
                        self.push(Entry::Attached { names });
                    }
                    self.push(Entry::User(prompt));
                    self.begin();
                }
                Record::Event { event, .. } => self.apply(event),
                _ => {}
            }
            used = at + 1;
        }
        used
    }

    fn left_unfinished(&mut self) {
        if matches!(self.status(), SessionStatus::Running | SessionStatus::Thinking | SessionStatus::NeedsYou) {
            let before = self.before();
            self.observe(&EngineEvent::Cancelled);
            self.follow_agents(&EngineEvent::Cancelled);
            self.settle(ToolState::Failed);
            self.tell_since(before, None);
        }
    }
}
