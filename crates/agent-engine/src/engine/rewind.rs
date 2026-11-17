use domain::{EngineEvent, Rewound, SessionId};

use super::job::Job;
use super::phases::load_session::LoadedSession;
use super::record::Session;
use super::stop::Stop;
use crate::checkpoints::{self, Checkpoint};
use crate::tools::Workspace;
use crate::transcript::Message;

impl Job {
    pub(super) fn open_checkpoint(&self, session: &SessionId, history: &[Message]) -> Checkpoint {
        let kept = self.sessions.load_checkpoints(&self.request.session.cwd, session).map(|all| all.len()).unwrap_or(0);
        self.gate.emit(EngineEvent::Checkpoint(u32::try_from(kept).unwrap_or(u32::MAX)));
        Checkpoint::open(&self.request.prompt.text, history)
    }

    pub(super) fn keep_checkpoint(&self, session: &SessionId, mut checkpoint: Checkpoint, workspace: &Workspace) {
        checkpoint.files = workspace.services.files.as_ref().map(|files| files.take()).unwrap_or_default();
        if let Err(error) = self.sessions.add_checkpoint(&self.request.session.cwd, session, checkpoint) {
            tracing::warn!(%error, "this run's checkpoint was not kept; it cannot be rewound to");
        }
    }

    pub(super) fn branch(&mut self, session: &LoadedSession, index: u32) -> Stop {
        let branched = SessionId(uuid::Uuid::new_v4().to_string());
        let made = self.sessions.load_checkpoints(&self.request.session.cwd, &session.id).and_then(|all| {
            let (prompt, kept, carried) = checkpoints::branch(&all, index as usize, &session.history)?;
            self.sessions.save_for(&self.request.session.cwd, &branched, &kept)?;
            self.sessions.save_checkpoints(&self.request.session.cwd, &branched, &carried)?;
            Ok(prompt)
        });
        match made {
            Ok(prompt) => {
                let recorder = self.record(&branched, Session::Kept);
                self.gate.emit(EngineEvent::SessionStarted(branched.clone()));
                recorder.opened(self.run_info(&self.request.session.cwd, None));
                self.gate.emit(EngineEvent::Branched { from: session.id.clone(), checkpoint: index, prompt });
                Stop::completed(None)
            }
            Err(error) => Stop::failed(format!("Could not branch: {error}")),
        }
    }

    pub(super) fn rewind(&mut self, session: LoadedSession, index: u32) -> Stop {
        let LoadedSession { id, mut history } = session;
        self.record(&id, Session::Kept).opened(self.run_info(&self.request.session.cwd, None));
        let rewound = self.sessions.load_checkpoints(&self.request.session.cwd, &id).and_then(|mut all| {
            let (target, restored, conversation) = checkpoints::rewind(&mut all, index as usize, &mut history)?;
            self.sessions.save_checkpoints(&self.request.session.cwd, &id, &all)?;
            if conversation {
                self.sessions.save_for(&self.request.session.cwd, &id, &history)?;
            }
            Ok(Rewound { checkpoint: index, prompt: target.prompt, restored, conversation })
        });
        match rewound {
            Ok(rewound) => {
                self.gate.emit(EngineEvent::Rewound(rewound));
                Stop::completed(None)
            }
            Err(error) => Stop::failed(format!("Could not rewind: {error}")),
        }
    }
}
