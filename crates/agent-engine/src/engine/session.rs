use std::sync::Arc;

use domain::trace::RunInfo;
use domain::{EngineEvent, ModelOption, SessionId};
use platform::sync::Locked;

use super::job::Job;
use super::record::{Recorder, Session};
use super::stop::Stop;
use crate::agent::Keep;
use crate::boundary::Observer;
use crate::meter::{Ledger, Metered};
use crate::model::ModelApi;
use crate::prompt::GitSnapshot;
use crate::sessions::Hold;
use crate::transcript::Message;

impl Job {
    pub(super) fn hold(&mut self, held: Option<Hold>) {
        *self.held.locked() = held;
        self.watch();
    }

    fn watch(&self) {
        let (recorder, held) = (self.recorder.clone(), Arc::clone(&self.held));
        let recorded_then_released: Observer = Arc::new(move |event: &EngineEvent| {
            if let Some(recorder) = &recorder {
                recorder.see(event);
            }
            if event.is_terminal() {
                held.locked().take();
            }
        });
        self.gate.observe(recorded_then_released);
    }

    pub(super) fn record(&mut self, session: &SessionId, state: Session) -> Arc<Recorder> {
        let recorder = Recorder::start(Arc::clone(&self.sessions), &self.request, session, state);
        self.recorder = Some(Arc::clone(&recorder));
        self.watch();
        recorder
    }

    pub(super) fn open_session(&mut self, session: &SessionId) {
        self.live.enter(session);
        let created_by_this_run = self.request.session.resume.is_none() || self.request.session.fork;
        if created_by_this_run && self.request.session.command.is_none() {
            self.record(session, Session::NotYetCreated).starts_unannounced();
        } else {
            self.record(session, Session::Kept);
            self.gate.emit(EngineEvent::SessionStarted(session.clone()));
        }
    }

    pub(super) fn signed_in(&self) -> Result<Arc<dyn ModelApi>, Stop> {
        let api = self.source.api(&self.request).map_err(Stop::Model)?;
        if let Some(recorder) = &self.recorder {
            recorder.announce(&self.gate);
        }
        Ok(api)
    }

    pub(super) fn metered(&self, api: Arc<dyn ModelApi>, ledger: &Arc<Ledger>, model: ModelOption) -> Metered {
        Metered {
            api,
            ledger: Arc::clone(ledger),
            provider: self.request.session.model.provider,
            gate: self.gate.clone(),
            model,
            calls: self.recorder.as_ref().map(|recorder| recorder.calls()),
            budget: self.request.policy.budget,
        }
    }

    pub(super) fn save(&self, session: &SessionId, history: &[Message]) -> Result<(), String> {
        match &self.recorder {
            Some(recorder) => recorder.kept(history),
            None => self.sessions.save_for(&self.request.session.cwd, session, history),
        }
    }

    pub(super) fn kept_at_every_transition(&self) -> Option<Keep> {
        let recorder = self.recorder.clone()?;
        Some(Arc::new(move |history: &[Message]| {
            if let Err(error) = recorder.kept(history) {
                tracing::warn!(%error, "the conversation was not saved at this step; it is saved again at the next");
            }
        }))
    }

    pub(super) fn run_info(&self, cwd: &std::path::Path, git: Option<&GitSnapshot>) -> RunInfo {
        RunInfo {
            model: self.request.session.model.id.clone(),
            provider: format!("{:?}", self.request.session.model.provider),
            permission: format!("{:?}", self.request.policy.permission),
            effort: format!("{:?}", self.request.session.effort),
            cwd: cwd.display().to_string(),
            branch: git.map(|git| git.branch.clone()),
            recent: git.map(|git| git.recent.clone()),
            status: git.map(|git| git.status.clone()),
        }
    }
}
