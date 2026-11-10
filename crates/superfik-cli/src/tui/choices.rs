//! The session's model, permission mode and effort: picked once the
//! models arrive, changed by the user, kept in the owner's settings.

use superfik_core::command::mode_label;
use superfik_core::model::choose_model;
use superfik_sdk::{Effort, Model, PermissionMode};

use super::app::App;
use super::settings;
use super::transcript::Transcript;

impl App {
    pub(super) fn ready(&mut self, models: Result<Vec<Model>, String>, saved: &serde_json::Value) {
        self.loading = false;
        let saved = settings::read(saved);
        if !self.mode_given {
            self.mode = saved.mode.unwrap_or(self.mode);
        }
        self.effort = saved.effort.unwrap_or(self.effort);
        match models {
            Ok(models) => self.models = models,
            Err(error) => return self.transcript.error(format!("Could not list the models: {error}")),
        }
        if self.session.is_some() {
            return;
        }
        let wanted = self.wanted.clone().or(saved.model);
        match choose_model(&self.models, wanted.as_deref()) {
            Some(model) => self.start_session(model),
            None => {
                self.transcript.notice("No model yet. Connect a provider to start:");
                self.login();
            }
        }
    }

    fn start_session(&mut self, model: Model) {
        let mut session = self.superfik.session(self.cwd.clone(), model);
        if let Some(id) = self.resume.take() {
            session = session.resume(id);
        }
        session.set_permission(self.mode);
        session.set_effort(self.effort);
        self.session = Some(session);
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
        if let Some(session) = &mut self.session {
            session.set_permission(mode);
        }
        if let Some(run) = &self.run {
            let _ = run.set_permission(mode);
        }
        settings::remember(&self.superfik, "mode", mode_label(mode));
    }

    pub fn set_model(&mut self, model: Model) {
        settings::remember(&self.superfik, "model", &model.id);
        self.transcript.notice(format!("Model: {}", model.label));
        match &mut self.session {
            Some(session) => session.set_model(model),
            None => self.start_session(model),
        }
    }

    pub(super) fn set_effort_from(&mut self, effort: Option<Effort>) {
        let Some(effort) = effort else {
            return self.transcript.notice(format!("Effort: {}", self.effort.cli_value()));
        };
        self.effort = effort;
        if let Some(session) = &mut self.session {
            session.set_effort(effort);
        }
        settings::remember(&self.superfik, "effort", effort.cli_value());
        self.transcript.notice(format!("Effort: {}", effort.cli_value()));
    }

    pub(super) fn new_session(&mut self) {
        if let Some(model) = self.model().cloned() {
            self.start_session(model);
        }
        self.transcript = Transcript::default();
        self.queued.clear();
        self.scroll = 0;
    }

    pub(super) fn compact(&mut self) {
        if self.busy() {
            return self.transcript.notice("Wait for the agent to finish before compacting.");
        }
        let Some(session) = &self.session else { return };
        let started = session.compact();
        self.transcript.notice("Compacting the conversation…");
        self.follow(started);
    }
}
