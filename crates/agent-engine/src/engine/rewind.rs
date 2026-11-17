use std::sync::Arc;

use domain::{EngineEvent, Rewound, SessionId};

use super::job::Job;
use super::phases::load_session::LoadedSession;
use super::record::Session;
use super::stop::Stop;
use crate::checkpoints::{self, Checkpoint};
use crate::tools::Workspace;
use crate::transcript::Message;

impl Job {
    pub(super) fn open_checkpoint(
        &self,
        session: &SessionId,
        history: &[Message],
        workspace: &Workspace,
    ) -> Checkpoint {
        let kept = self.sessions.load_checkpoints(&self.request.session.cwd, session).map(|all| all.len()).unwrap_or(0);
        self.gate.emit(EngineEvent::Checkpoint(u32::try_from(kept).unwrap_or(u32::MAX)));
        let checkpoint = Checkpoint::open(&self.request.prompt.text, history);
        if let Some(files) = &workspace.services.files {
            let (sessions, cwd, id) = (Arc::clone(&self.sessions), self.request.session.cwd.clone(), session.clone());
            let opened = checkpoint.clone();
            files.journaled(Arc::new(move |kept| {
                let so_far =
                    Checkpoint { files: kept.files.clone(), not_kept: kept.not_kept.clone(), ..opened.clone() };
                if let Err(error) = sessions.add_checkpoint(&cwd, &id, so_far) {
                    tracing::warn!(%error, "a file's state before this run's write was not kept on disk");
                }
            }));
        }
        checkpoint
    }

    pub(super) fn keep_checkpoint(&self, session: &SessionId, mut checkpoint: Checkpoint, workspace: &Workspace) {
        let kept = workspace.services.files.as_ref().map(|files| files.take()).unwrap_or_default();
        (checkpoint.files, checkpoint.not_kept) = (kept.files, kept.not_kept);
        checkpoint.left = checkpoints::left_by_the_agent(&checkpoint.files);
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
        let cwd = &self.request.session.cwd;
        let rewound = self.sessions.load_checkpoints(cwd, &id).and_then(|mut all| {
            let checkpoints::Rewinding { target, restored, skipped, conversation } =
                checkpoints::rewind(&mut all, index as usize, &mut history)?;
            let unsaved = |what: &str, error: String| {
                format!(
                    "{} restored, but {what} could not be saved, so the rewind can be tried again: {error}",
                    files_restored(restored.len())
                )
            };
            if conversation {
                self.sessions.save_for(cwd, &id, &history).map_err(|error| unsaved("the conversation", error))?;
            }
            self.sessions.save_checkpoints(cwd, &id, &all).map_err(|error| unsaved("the checkpoints", error))?;
            Ok(Rewound { checkpoint: index, prompt: target.prompt, restored, conversation, skipped })
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

fn files_restored(count: usize) -> String {
    match count {
        1 => "1 file was".to_owned(),
        count => format!("{count} files were"),
    }
}
