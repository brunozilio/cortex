//! The chat's state and what it does: send prompts (queued while the agent
//! works), follow the running run, run the chat's own `/commands`. Slow
//! calls run on threads and come back as [`Msg`]s. Choosing the model,
//! mode and effort is in `choices`, connecting a provider in `login`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use superfik_core::command::Command;
use superfik_sdk::{Effort, EngineEvent, Model, PermissionMode, Run, RunControl, Session, SessionId, Superfik};

use super::ask::Pending;
use super::input::Input;
use super::login::SecretPrompt;
use super::transcript::{Entry, Transcript};

pub enum Msg {
    Ready { models: Result<Vec<Model>, String>, settings: serde_json::Value },
    Event(superfik_core::RunId, EngineEvent),
    Skills(Vec<(String, String)>),
    Connected(String),
    Notice(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Model,
    Mode,
    Login,
}

#[derive(Debug)]
pub struct Picker {
    pub kind: PickerKind,
    pub title: String,
    pub items: Vec<(String, String)>,
    pub cursor: usize,
}

/// How long a first Ctrl+C waits for the second that quits.
const QUIT_WINDOW: Duration = Duration::from_secs(2);

pub struct App {
    pub superfik: Superfik,
    pub cwd: PathBuf,
    pub session: Option<Session>,
    pub transcript: Transcript,
    pub input: Input,
    pub ask: Option<Pending>,
    pub secret: Option<SecretPrompt>,
    pub run: Option<RunControl>,
    pub started: Option<Instant>,
    pub queued: Vec<String>,
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub picker: Option<Picker>,
    pub scroll: u16,
    pub quit: bool,
    pub quit_armed: Option<Instant>,
    pub loading: bool,
    pub(super) wanted: Option<String>,
    pub(super) resume: Option<SessionId>,
    pub(super) mode_given: bool,
    pub(super) mode: PermissionMode,
    pub(super) effort: Effort,
    pub(super) tx: Sender<Msg>,
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
            secret: None,
            run: None,
            started: None,
            queued: Vec::new(),
            models: Vec::new(),
            skills: Vec::new(),
            picker: None,
            scroll: 0,
            quit: false,
            quit_armed: None,
            loading: true,
            wanted,
            resume: None,
            mode_given: mode.is_some(),
            mode: mode.unwrap_or_default(),
            effort: Effort::Medium,
            tx,
            rx,
        };
        app.load(true);
        app
    }

    /// Continues `session` (from `superfik -c`) once a model is chosen.
    pub fn resuming(mut self, session: Option<SessionId>) -> Self {
        if session.is_some() {
            self.transcript.notice("Continuing the last conversation in this folder.");
        }
        self.resume = session;
        self
    }

    /// Asks for the settings and models (and the skills, at first).
    pub(super) fn load(&mut self, skills: bool) {
        self.loading = true;
        let (superfik, tx, cwd) = (self.superfik.clone(), self.tx.clone(), self.cwd.clone());
        std::thread::spawn(move || {
            let settings = superfik.settings().unwrap_or_else(|_| serde_json::json!({}));
            let models = superfik.models().map_err(|error| error.to_string());
            let _ = tx.send(Msg::Ready { models, settings });
            if skills && let Ok(found) = superfik.skills(&cwd) {
                let _ = tx.send(Msg::Skills(found.into_iter().map(|s| (s.name, s.description)).collect()));
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
            Msg::Connected(provider) => {
                self.transcript.notice(format!("Connected {provider}."));
                self.load(false);
            }
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Error(text) => self.transcript.error(text),
        }
    }

    fn event(&mut self, run: superfik_core::RunId, event: EngineEvent) {
        if self.run.as_ref().is_none_or(|current| current.id() != run) {
            return;
        }
        match &event {
            EngineEvent::Asked(ask) => self.ask = Some(Pending::new(ask.clone())),
            EngineEvent::AskWithdrawn(id) if self.ask.as_ref().is_some_and(|p| &p.ask.id == id) => self.ask = None,
            EngineEvent::SessionStarted(id) => super::settings::remember_session(&self.superfik, &self.cwd, id),
            _ => {}
        }
        self.transcript.apply(&event);
        if event.is_terminal() {
            self.run = None;
            self.ask = None;
            self.started = None;
            if matches!(event, EngineEvent::Cancelled) {
                self.unqueue();
            } else if !self.queued.is_empty() {
                let next = self.queued.remove(0);
                self.send(next);
            }
        }
    }

    /// Sends what was typed: a chat command, or a prompt for the agent
    /// (queued while it works).
    pub fn submit(&mut self) {
        let text = self.input.text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.input.take();
        if let Some(command) = Command::parse(&text) {
            return super::commands::run(self, command);
        }
        if self.busy() {
            self.queued.push(text);
            return;
        }
        self.send(text);
    }

    fn send(&mut self, text: String) {
        let Some(session) = &self.session else {
            self.input.set(&text);
            return self.transcript.error("No model yet: /login connects a provider.");
        };
        let started = session.send(&text);
        self.transcript.push(Entry::User(text));
        self.follow(started);
    }

    /// Puts what was queued back in the prompt, so an interrupt loses nothing.
    fn unqueue(&mut self) {
        if self.queued.is_empty() {
            return;
        }
        let mut text = std::mem::take(&mut self.queued).join("\n\n");
        if !self.input.is_empty() {
            text = format!("{text}\n\n{}", self.input.text());
        }
        self.input.set(&text);
    }

    pub(super) fn follow(&mut self, started: Result<Run, superfik_sdk::Error>) {
        match started {
            Ok(run) => {
                self.run = Some(run.control());
                self.started = Some(Instant::now());
                self.transcript.begin();
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

    /// Ctrl+C: stops the run, else clears the prompt, else quits on a
    /// second press.
    pub fn interrupt(&mut self) {
        if self.busy() {
            self.cancel();
        } else if !self.input.is_empty() {
            self.input.clear();
        } else if self.quit_armed.is_some_and(|at| at.elapsed() < QUIT_WINDOW) {
            self.quit = true;
        } else {
            self.quit_armed = Some(Instant::now());
        }
    }

    /// Whether the status line should say "Ctrl+C again to quit".
    pub fn quitting(&self) -> bool {
        self.quit_armed.is_some_and(|at| at.elapsed() < QUIT_WINDOW)
    }

    pub fn answer(&mut self, decision: superfik_sdk::Decision) {
        if let (Some(pending), Some(run)) = (self.ask.take(), &self.run)
            && let Err(error) = run.answer(pending.ask.id, decision)
        {
            self.transcript.error(error.to_string());
        }
    }

    pub(super) fn spawn(&self, work: impl FnOnce(&Superfik) -> Msg + Send + 'static) {
        let (superfik, tx) = (self.superfik.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(work(&superfik));
        });
    }
}
