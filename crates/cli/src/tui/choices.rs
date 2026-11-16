use domain::command::mode_label;
use domain::model::choose_model;
use sdk::{Effort, Model, PermissionMode};

use super::app::App;
use super::settings;
use super::transcript::Transcript;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Model,
    Mode,
    Login,
    Rewind,
    Resume,
    Logout,
    OutputStyle,
    Config,
    Effort,
    Role(domain::model_roles::ModelRole),
    Account { removing: bool },
    PortBusy,
}

const MODES: [(PermissionMode, &str); 4] = [
    (PermissionMode::Ask, "Ask before edits and commands"),
    (PermissionMode::AcceptEdits, "Edit files freely, ask before commands"),
    (PermissionMode::Plan, "Read and plan only"),
    (PermissionMode::Bypass, "Never ask (a safety check still grades risky actions)"),
];

#[derive(Debug)]
pub struct Picker {
    pub kind: PickerKind,
    pub title: String,
    pub items: Vec<(String, String)>,
    pub cursor: usize,
}

impl App {
    pub(super) fn ready(&mut self, models: Result<Vec<Model>, String>, saved: &serde_json::Value) {
        self.catalog.loading = false;
        if self.catalog.models.is_empty() && self.session.is_none() {
            self.input.set_history(settings::history(saved, &self.cwd));
            if saved["vim"] == "on" {
                self.vim = Some(super::vim::Vim::default());
            }
            self.output_style = saved["outputStyle"].as_str().map(str::to_owned);
            self.roles = super::model_roles::saved(saved);
            self.fallbacks = super::fallbacks::saved(saved);
            self.status.command = saved["statusLine"].as_str().filter(|c| !c.is_empty()).map(str::to_owned);
            if let Some(id) = &self.resume {
                self.title = settings::title(saved, &self.cwd, id);
            }
        }
        let first = self.catalog.models.is_empty() && self.session.is_none();
        let saved = settings::read(saved);
        for problem in saved.problems.iter().filter(|_| first) {
            self.transcript.notice(problem.clone());
        }
        if !self.mode_given {
            self.mode = saved.mode.unwrap_or(self.mode);
        }
        self.effort = saved.effort.unwrap_or(self.effort);
        self.auto_effort = saved.auto;
        match models {
            Ok(models) => self.catalog.models = models,
            Err(error) => return self.transcript.error(format!("Could not list the models: {error}")),
        }
        if self.session.is_some() {
            return;
        }
        let wanted = self.wanted.clone().or(saved.model);
        match choose_model(&self.catalog.models, wanted.as_deref()) {
            Some(model) => self.start_session(model),
            None => {
                self.transcript.passing("No model yet. Connect a provider to start:");
                self.login();
            }
        }
    }

    fn start_session(&mut self, model: Model) {
        let mut session = self.sdk.session(self.cwd.clone(), model);
        let resumed = self.resume.take();
        if let Some(id) = resumed.clone() {
            session = session.resume(id);
            if std::mem::take(&mut self.fork_resumed) {
                session.fork_next();
            }
        }
        session.set_permission(self.mode);
        session.set_effort(self.effort);
        if self.auto_effort {
            session.set_auto_pick();
        }
        for dir in &self.added_dirs {
            session.add_dir(dir.clone());
        }
        session.set_output_style(self.output_style.clone());
        session.set_roles(self.roles.clone());
        session.set_fallbacks(self.fallbacks.clone());
        session.set_headless(self.headless.clone());
        self.session = Some(session);
        if resumed.is_some() {
            self.recap();
        }
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
        if let Some(session) = &mut self.session {
            session.set_permission(mode);
        }
        if let Some(run) = &self.turn.run {
            let _ = run.set_permission(mode);
        }
        self.saver.remember("mode", mode_label(mode));
    }

    pub(super) fn model_picker(&mut self) {
        if self.catalog.models.is_empty() {
            return self.transcript.notice(concat!(
                "No models yet. Run `",
                brand::command!(),
                " login` to connect a provider."
            ));
        }
        let current = self.model().map(|m| m.id.clone());
        let models = &self.catalog.models;
        let items = models.iter().map(|m| (m.label.clone(), m.id.clone())).collect();
        let cursor = models.iter().position(|m| Some(&m.id) == current.as_ref()).unwrap_or(0);
        self.overlays.picker = Some(Picker { kind: PickerKind::Model, title: "Model".into(), items, cursor });
    }

    pub(super) fn set_model_named(&mut self, wanted: &str) {
        match self.catalog.models.iter().find(|m| m.is_named(wanted)).cloned() {
            Some(model) => self.set_model(model),
            None => self.transcript.error(format!("No model named {wanted}. /model lists them.")),
        }
    }

    pub(super) fn mode_picker(&mut self) {
        let cursor = MODES.iter().position(|(mode, _)| *mode == self.mode).unwrap_or(0);
        let items = MODES.iter().map(|(mode, about)| (mode_label(*mode).to_owned(), (*about).to_owned())).collect();
        self.overlays.picker = Some(Picker { kind: PickerKind::Mode, title: "Permission mode".into(), items, cursor });
    }

    pub(super) fn switch_mode(&mut self, mode: PermissionMode) {
        self.set_mode(mode);
        self.transcript.notice(format!("Mode: {}", mode_label(mode)));
    }

    pub fn set_model(&mut self, model: Model) {
        self.saver.remember("model", &model.id);
        self.transcript.notice(format!("Model: {}", model.label));
        match &mut self.session {
            Some(session) => session.set_model(model),
            None => self.start_session(model),
        }
    }

    pub(super) fn effort_picker(&mut self) {
        let items = Effort::ALL.iter().map(|e| (e.cli_value().to_owned(), effort_about(*e).to_owned())).collect();
        let cursor = Effort::ALL.iter().position(|e| *e == self.effort).unwrap_or(0);
        self.overlays.picker = Some(Picker { kind: PickerKind::Effort, title: "Effort".into(), items, cursor });
    }

    pub(super) fn set_effort_from(&mut self, effort: Option<Effort>) {
        let Some(effort) = effort else {
            return self.transcript.notice(format!("Effort: {}", self.effort_label()));
        };
        self.effort = effort;
        self.auto_effort = false;
        if let Some(session) = &mut self.session {
            session.set_effort(effort);
        }
        self.saver.remember("effort", effort.cli_value());
        self.transcript.notice(format!("Effort: {}", effort.cli_value()));
    }

    pub(super) fn set_auto_effort(&mut self) {
        self.auto_effort = true;
        if let Some(session) = &mut self.session {
            session.set_auto_pick();
        }
        self.saver.remember("effort", super::settings::AUTO_EFFORT);
        self.transcript.notice(
            "Effort: auto. Each request picks the model and effort it needs; /effort <level> goes back to one level.",
        );
    }

    pub(super) fn new_session(&mut self) {
        if let Some(model) = self.model().cloned() {
            self.start_session(model);
        }
        self.transcript = Transcript::default();
        self.turn.queued.clear();
        self.turn.checkpoints.clear();
        self.title = None;
        self.ghost = None;
        self.scroll = 0;
    }

    pub(super) fn compact(&mut self, focus: &str) {
        if self.busy() {
            return self.transcript.notice("Wait for the agent to finish before compacting.");
        }
        let Some(session) = &self.session else { return };
        let started = session.compact_on(focus);
        self.transcript.notice("Compacting the conversation…");
        self.follow(started);
    }
}

fn effort_about(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "Quickest answers, least reasoning",
        Effort::Medium => "Balanced",
        Effort::High => "More reasoning for harder work",
        Effort::XHigh => "Extended reasoning",
        Effort::Max => "As much reasoning as the model allows",
    }
}
