//! The chat's state and what it does: send prompts, follow the running
//! run, run the chat's own `/commands`. Slow calls run on threads and
//! come back as [`Msg`]s.

use std::path::PathBuf;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use superfik_core::command::{Command, mode_label};
use superfik_core::model::choose_model;
use superfik_sdk::{Effort, EngineEvent, Model, PermissionMode, Run, RunControl, Session, Superfik};

use super::ask::Pending;
use super::input::Input;
use super::settings;
use super::transcript::{Entry, Transcript};

pub enum Msg {
    Ready { models: Result<Vec<Model>, String>, settings: serde_json::Value },
    Event(superfik_core::RunId, EngineEvent),
    Skills(Vec<(String, String)>),
    Notice(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Model,
    Mode,
}

#[derive(Debug)]
pub struct Picker {
    pub kind: PickerKind,
    pub title: String,
    pub items: Vec<(String, String)>,
    pub cursor: usize,
}

pub struct App {
    pub superfik: Superfik,
    pub cwd: PathBuf,
    pub session: Option<Session>,
    pub transcript: Transcript,
    pub input: Input,
    pub ask: Option<Pending>,
    pub run: Option<RunControl>,
    pub started: Option<Instant>,
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub picker: Option<Picker>,
    pub scroll: u16,
    pub quit: bool,
    pub loading: bool,
    wanted: Option<String>,
    mode_given: bool,
    mode: PermissionMode,
    effort: Effort,
    tx: Sender<Msg>,
    pub rx: Receiver<Msg>,
}

impl App {
    pub fn new(superfik: Superfik, cwd: PathBuf, wanted: Option<String>, mode: Option<PermissionMode>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut app = Self {
            superfik,
            cwd,
            session: None,
            transcript: Transcript::default(),
            input: Input::default(),
            ask: None,
            run: None,
            started: None,
            models: Vec::new(),
            skills: Vec::new(),
            picker: None,
            scroll: 0,
            quit: false,
            loading: true,
            wanted,
            mode_given: mode.is_some(),
            mode: mode.unwrap_or_default(),
            effort: Effort::Medium,
            tx,
            rx,
        };
        app.load();
        app
    }

    fn load(&mut self) {
        let (superfik, tx, cwd) = (self.superfik.clone(), self.tx.clone(), self.cwd.clone());
        std::thread::spawn(move || {
            let settings = superfik.settings().unwrap_or_else(|_| serde_json::json!({}));
            let models = superfik.models().map_err(|error| error.to_string());
            let _ = tx.send(Msg::Ready { models, settings });
            if let Ok(skills) = superfik.skills(&cwd) {
                let _ = tx.send(Msg::Skills(skills.into_iter().map(|s| (s.name, s.description)).collect()));
            }
        });
    }

    pub fn busy(&self) -> bool {
        self.run.is_some()
    }

    pub fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub fn effort(&self) -> Effort {
        self.effort
    }

    pub fn model(&self) -> Option<&Model> {
        self.session.as_ref().map(Session::model)
    }

    pub fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Ready { models, settings } => self.ready(models, &settings),
            Msg::Event(run, event) => self.event(run, event),
            Msg::Skills(skills) => self.skills = skills,
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Error(text) => self.transcript.error(text),
        }
    }

    fn ready(&mut self, models: Result<Vec<Model>, String>, settings: &serde_json::Value) {
        self.loading = false;
        let saved = settings::read(settings);
        let wanted = self.wanted.clone().or(saved.model);
        if !self.mode_given {
            self.mode = saved.mode.unwrap_or(self.mode);
        }
        self.effort = saved.effort.unwrap_or(self.effort);
        match models {
            Ok(models) => {
                self.models = models;
                match choose_model(&self.models, wanted.as_deref()) {
                    Some(model) => self.start_session(model),
                    None => self.transcript.error("No model is available. Run `superfik login` to connect a provider."),
                }
            }
            Err(error) => self.transcript.error(format!("Could not list the models: {error}")),
        }
    }

    fn start_session(&mut self, model: Model) {
        let mut session = self.superfik.session(self.cwd.clone(), model);
        session.set_permission(self.mode);
        session.set_effort(self.effort);
        self.session = Some(session);
    }

    fn event(&mut self, run: superfik_core::RunId, event: EngineEvent) {
        if self.run.as_ref().is_none_or(|current| current.id() != run) {
            return;
        }
        match &event {
            EngineEvent::Asked(ask) => self.ask = Some(Pending::new(ask.clone())),
            EngineEvent::AskWithdrawn(id) if self.ask.as_ref().is_some_and(|p| &p.ask.id == id) => self.ask = None,
            _ => {}
        }
        self.transcript.apply(&event);
        if event.is_terminal() {
            self.run = None;
            self.ask = None;
            self.started = None;
        }
    }

    /// Sends what was typed: a chat command, or a prompt for the agent.
    pub fn submit(&mut self) {
        let text = self.input.text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        if let Some(command) = Command::parse(&text) {
            self.input.take();
            return self.command(command);
        }
        if self.busy() {
            return self.transcript.notice("The agent is still working: Esc interrupts it.");
        }
        let Some(session) = &self.session else {
            return self.transcript.error("No model yet. Run `superfik login`, or wait for the models to load.");
        };
        let started = session.send(&text);
        self.input.take();
        self.transcript.push(Entry::User(text));
        self.follow(started);
    }

    fn follow(&mut self, started: Result<Run, superfik_sdk::Error>) {
        match started {
            Ok(run) => {
                self.run = Some(run.control());
                self.started = Some(Instant::now());
                self.scroll = 0;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let id = run.id();
                    for event in run {
                        if tx.send(Msg::Event(id, event)).is_err() {
                            break;
                        }
                    }
                });
            }
            Err(error) => self.transcript.error(error.to_string()),
        }
    }

    pub fn cancel(&mut self) {
        if let Some(run) = &self.run
            && let Err(error) = run.cancel()
        {
            self.transcript.error(error.to_string());
        }
    }

    pub fn answer(&mut self, decision: superfik_sdk::Decision) {
        if let (Some(pending), Some(run)) = (self.ask.take(), &self.run)
            && let Err(error) = run.answer(pending.ask.id, decision)
        {
            self.transcript.error(error.to_string());
        }
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

    fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
        if let Some(session) = &mut self.session {
            session.set_effort(effort);
        }
        settings::remember(&self.superfik, "effort", effort.cli_value());
        self.transcript.notice(format!("Effort: {}", effort.cli_value()));
    }

    fn command(&mut self, command: Command) {
        super::commands::run(self, command);
    }

    pub(super) fn spawn(&self, work: impl FnOnce(&Superfik) -> Msg + Send + 'static) {
        let (superfik, tx) = (self.superfik.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(work(&superfik));
        });
    }

    pub(super) fn new_session(&mut self) {
        if let Some(model) = self.model().cloned() {
            self.start_session(model);
        }
        self.transcript = Transcript::default();
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

    pub(super) fn set_effort_from(&mut self, effort: Option<Effort>) {
        match effort {
            Some(effort) => self.set_effort(effort),
            None => self.transcript.notice(format!("Effort: {}", self.effort.cli_value())),
        }
    }
}
