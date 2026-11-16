use super::{Change, Entry, Transcript};
use crate::EngineEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Checkpoint {
    pub index: u32,
    pub entry: usize,
}

impl Transcript {
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    pub fn take_draft(&mut self) -> Option<String> {
        self.draft.take()
    }

    pub(super) fn mark(&mut self, event: &EngineEvent) {
        if !self.record {
            return;
        }
        match event {
            EngineEvent::Checkpoint(index) => {
                let prompted = self.entries.iter().rposition(|entry| matches!(entry, Entry::User(_)));
                let Some(entry) = prompted else { return };
                self.checkpoints.retain(|known| known.index < *index);
                self.checkpoints.push(Checkpoint { index: *index, entry });
                self.tell(Change::Checkpoints);
            }
            EngineEvent::Rewound(rewound) => {
                let told = Entry::Rewound { restored: rewound.restored.len(), conversation: rewound.conversation };
                self.back_to(rewound.checkpoint, &rewound.prompt, told);
            }
            EngineEvent::Branched { checkpoint, prompt, .. } => self.back_to(*checkpoint, prompt, Entry::Branched),
            _ => {}
        }
    }

    fn back_to(&mut self, checkpoint: u32, prompt: &str, told: Entry) {
        let Some(at) = self.checkpoints.iter().position(|known| known.index == checkpoint) else { return };
        let entry = self.checkpoints[at].entry;
        self.entries.truncate(entry);
        self.checkpoints.truncate(at);
        self.cut(entry);
        self.tell(Change::Checkpoints);
        self.push(told);
        self.draft = Some(prompt.to_owned());
        self.tell(Change::Draft);
    }
}
