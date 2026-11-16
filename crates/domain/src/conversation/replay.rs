use super::{Entry, SessionStatus, ToolState, Transcript};
use crate::EngineEvent;
use crate::trace::Record;

impl Transcript {
    pub fn replay(&mut self, records: &[Record]) {
        let mut shown = None;
        for record in records {
            match record {
                Record::Shown { text, attached } => shown = Some((text.clone(), attached.clone())),
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
        }
        self.left_unfinished();
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
