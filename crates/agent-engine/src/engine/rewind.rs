use std::time::Instant;

use domain::{EngineEvent, FailureKind, Rewound, SessionId};

use super::job::Job;
use super::record::Session;
use crate::checkpoints::{self, Checkpoint};
use crate::transcript::Message;

impl Job {
    pub(super) fn open_checkpoint(&self, session: &SessionId, history: &[Message]) -> Checkpoint {
        let kept = self.sessions.load_checkpoints(&self.request, session).map(|all| all.len()).unwrap_or(0);
        self.gate.emit(EngineEvent::Checkpoint(u32::try_from(kept).unwrap_or(u32::MAX)));
        Checkpoint::open(&self.request.prompt, history)
    }

    pub(super) fn keep_checkpoint(&self, session: &SessionId, mut checkpoint: Checkpoint) {
        checkpoint.files = self.workspace.services.files.as_ref().map(|files| files.take()).unwrap_or_default();
        let saved = self.sessions.load_checkpoints(&self.request, session).and_then(|mut all| {
            all.push(checkpoint);
            self.sessions.save_checkpoints(&self.request, session, &all)
        });
        if let Err(error) = saved {
            tracing::warn!(%error, "this run's checkpoint was not kept; it cannot be rewound to");
        }
    }

    pub(super) fn branch(&mut self, session: &SessionId, history: &[Message], index: u32, started: Instant) {
        let branched = SessionId(uuid::Uuid::new_v4().to_string());
        let made = self.sessions.load_checkpoints(&self.request, session).and_then(|all| {
            let (prompt, kept, carried) = checkpoints::branch(&all, index as usize, history)?;
            self.sessions.save_for(&self.request, &branched, &kept)?;
            self.sessions.save_checkpoints(&self.request, &branched, &carried)?;
            Ok(prompt)
        });
        match made {
            Ok(prompt) => {
                let recorder = self.record(&branched, Session::Kept);
                self.gate.emit(EngineEvent::SessionStarted(branched.clone()));
                recorder.opened(self.run_info(None));
                self.gate.emit(EngineEvent::Branched { from: session.clone(), checkpoint: index, prompt });
                self.completed(started, None);
            }
            Err(error) => self.fail(&format!("Could not branch: {error}"), FailureKind::Other),
        }
    }

    pub(super) fn rewind(&mut self, session: &SessionId, mut history: Vec<Message>, index: u32, started: Instant) {
        self.record(session, Session::Kept).opened(self.run_info(None));
        let rewound = self.sessions.load_checkpoints(&self.request, session).and_then(|mut all| {
            let (target, restored, conversation) = checkpoints::rewind(&mut all, index as usize, &mut history)?;
            self.sessions.save_checkpoints(&self.request, session, &all)?;
            if conversation {
                self.sessions.save_for(&self.request, session, &history)?;
            }
            Ok(Rewound { checkpoint: index, prompt: target.prompt, restored, conversation })
        });
        match rewound {
            Ok(rewound) => {
                self.gate.emit(EngineEvent::Rewound(rewound));
                self.completed(started, None);
            }
            Err(error) => self.fail(&format!("Could not rewind: {error}"), FailureKind::Other),
        }
    }
}
