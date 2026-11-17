use app::Model;
use domain::model::choose_model;

use super::app::App;
use super::settings;
use super::transcript::Transcript;

impl App {
    pub(super) fn ready(&mut self, models: Result<Vec<Model>, String>, saved: &serde_json::Value) {
        self.catalog.loading = false;
        let first = self.catalog.models.is_empty() && self.session.is_none();
        if first {
            self.take_saved_surroundings(saved);
        }
        let saved = settings::read(saved);
        for problem in saved.problems.iter().filter(|_| first) {
            self.transcript.notice(problem.clone());
        }
        if self.session.is_none() {
            self.session.take_saved_choices(&saved);
        }
        match models {
            Ok(models) => self.catalog.models = models,
            Err(error) => return self.transcript.error(format!("Could not list the models: {error}")),
        }
        if self.session.is_some() {
            return;
        }
        let wanted = self.session.wanted.clone().or(saved.model);
        match choose_model(&self.catalog.models, wanted.as_deref()) {
            Some(model) => self.start_session(model),
            None => {
                self.transcript.passing("No model yet. Connect a provider to start:");
                super::sign_in::open(self);
            }
        }
    }

    fn take_saved_surroundings(&mut self, saved: &serde_json::Value) {
        self.prompt.input.set_history(settings::history(saved, &self.cwd));
        if saved["vim"] == "on" {
            self.prompt.vim = Some(super::vim::Vim::default());
        }
        self.session.output_style = saved["outputStyle"].as_str().map(str::to_owned);
        self.session.roles = super::session_choices::saved_roles(saved);
        self.session.fallbacks = super::session_choices::saved_fallbacks(saved);
        self.ui.status.command = saved["statusLine"].as_str().filter(|c| !c.is_empty()).map(str::to_owned);
        if let Some(id) = &self.session.resume {
            self.session.title = settings::title(saved, &self.cwd, id);
        }
    }

    pub(super) fn start_session(&mut self, model: Model) {
        let mut session = self.client.session(self.cwd.clone(), model);
        let resumed = self.session.resume.take();
        if let Some(id) = resumed.clone() {
            session = session.resume(id);
            if std::mem::take(&mut self.session.fork_resumed) {
                session.fork_next();
            }
        }
        self.session.adopt(session);
        if resumed.is_some() {
            self.recap();
        }
    }

    pub(super) fn new_session(&mut self) {
        if let Some(model) = self.model().cloned() {
            self.start_session(model);
        }
        self.transcript = Transcript::default();
        self.prompt.forget_the_conversation();
        self.turn.queued.clear();
        self.turn.checkpoints.clear();
        self.session.title = None;
        self.ui.scroll = 0;
    }

    pub(super) fn resume_picked(&mut self, cursor: usize) {
        let Some(picked) = self.catalog.conversations.get(cursor).cloned() else { return };
        self.session.resume = Some(picked.session);
        self.new_session();
        self.session.title = picked.title;
        self.transcript.notice("Continuing an earlier conversation.");
    }

    pub(super) fn compact(&mut self, focus: &str) {
        if self.busy() {
            return self.transcript.notice("Wait for the agent to finish before compacting.");
        }
        let Some(session) = self.session.live() else { return };
        let started = session.compact_on(focus);
        self.transcript.notice("Compacting the conversation…");
        self.follow(started);
    }
}
