use super::{Change, SessionStatus, Transcript};
use crate::{EngineEvent, FailureKind};

pub const RETRY_AFTER_SECONDS: u64 = 30;

impl Transcript {
    pub fn queued(&self) -> &[String] {
        &self.queue
    }

    pub fn steered(&mut self, text: &str) {
        self.queue.push(text.to_owned());
        self.tell(Change::Queue);
    }

    pub fn owes_retry(&mut self) -> bool {
        let dropped = self.status() == SessionStatus::Failed && self.failure() == Some(FailureKind::Unavailable);
        let owed = dropped && !self.retried;
        self.retried |= owed;
        owed
    }

    pub fn fresh(&mut self) {
        self.retried = false;
    }

    pub(super) fn line_up(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::Steered(text) => self.taken(text),
            EngineEvent::Unsteered(texts) if !texts.is_empty() => {
                texts.iter().for_each(|text| self.taken(text));
                self.draft = Some(texts.join("\n\n"));
                self.tell(Change::Draft);
            }
            _ => {}
        }
    }

    fn taken(&mut self, text: &str) {
        if let Some(at) = self.queue.iter().position(|waiting| waiting == text) {
            self.queue.remove(at);
            self.tell(Change::Queue);
        }
    }
}
