//! The chat's state and what it does: send prompts (queued while the agent
//! works), run the chat's own `/commands`. Slow calls run on threads and
//! come back as [`Msg`]s. Following a run is in `runs`, choosing the model,
//! mode and effort in `choices`, connecting a provider in `login`, `/btw`
//! in `side`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use superfik_core::command::Command;
use superfik_sdk::{Effort, EngineEvent, Model, PermissionMode, RunControl, Session, SessionId, Superfik};

use super::ask::Pending;
use super::choices::Picker;
use super::input::Input;
use super::login::SecretPrompt;
use super::transcript::Transcript;

pub enum Msg {
    Ready {
        models: Result<Vec<Model>, String>,
        settings: serde_json::Value,
    },
    Event(superfik_core::RunId, EngineEvent),
    Skills(Vec<(String, String)>),
    /// What `/reload` found: the skills to complete and what to say.
    Reloaded {
        skills: Vec<(String, String)>,
        summary: String,
    },
    Connected(String),
    /// What Ctrl+V found on the clipboard.
    Clipboard(Result<Option<superfik_sdk::Image>, String>),
    Changes(Result<super::git::Changes, String>),
    /// What `/diffs` asked git for.
    Diffs(super::git::Scope, Result<super::git::Changes, String>),
    Side {
        question: String,
        answer: Result<String, String>,
    },
    /// A side call the chat made by itself (`chores`).
    Chore(Result<Option<superfik_sdk::ChoreAnswer>, String>),
    Notice(String),
    Error(String),
}

/// How long a first Ctrl+C waits for the second that quits.
const QUIT_WINDOW: Duration = Duration::from_secs(2);

pub struct App {
    pub superfik: Superfik,
    pub cwd: PathBuf,
    pub session: Option<Session>,
    pub transcript: Transcript,
    pub input: Input,
    /// Animations; a `RefCell` because drawing advances them.
    pub motion: std::cell::RefCell<super::motion::Motion>,
    pub attachments: super::attach::Attachments,
    /// Ctrl+O: reasoning, outputs and diffs shown in full.
    pub expanded: bool,
    pub ask: Option<Pending>,
    pub secret: Option<SecretPrompt>,
    pub run: Option<RunControl>,
    pub started: Option<Instant>,
    pub queued: Vec<String>,
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub picker: Option<Picker>,
    /// A `/btw` question and its answer, shown until dismissed.
    pub side: Option<super::side::Side>,
    /// The conversation's name, once a side call gave one.
    pub title: Option<String>,
    /// The prompt you would likely type next, shown in the empty prompt.
    pub ghost: Option<String>,
    /// `/diffs`, full screen while open.
    pub diffs: Option<super::diff_view::DiffView>,
    pub scroll: u16,
    pub quit: bool,
    pub quit_armed: Option<Instant>,
    /// A first Esc with text typed; the second clears it.
    pub clear_armed: Option<Instant>,
    /// The highlighted `/` completion.
    pub suggestion: usize,
    pub loading: bool,
    pub(super) wanted: Option<String>,
    pub(super) resume: Option<SessionId>,
    pub(super) mode_given: bool,
    pub(super) mode: PermissionMode,
    pub(super) effort: Effort,
    pub(super) tx: Sender<Msg>,
    pub(super) saver: super::settings::Saver,
    pub rx: Receiver<Msg>,
}

impl App {
    pub fn new(superfik: Superfik, cwd: PathBuf, wanted: Option<String>, mode: Option<PermissionMode>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let saver = super::settings::Saver::start(&superfik);
        let mut app = Self {
            superfik,
            cwd,
            session: None,
            transcript: Transcript::default(),
            input: Input::default(),
            motion: std::cell::RefCell::new(super::motion::Motion::from_env()),
            attachments: Default::default(),
            expanded: false,
            ask: None,
            secret: None,
            run: None,
            started: None,
            queued: Vec::new(),
            models: Vec::new(),
            skills: Vec::new(),
            picker: None,
            side: None,
            title: None,
            ghost: None,
            diffs: None,
            scroll: 0,
            quit: false,
            quit_armed: None,
            clear_armed: None,
            suggestion: 0,
            loading: true,
            wanted,
            resume: None,
            mode_given: mode.is_some(),
            mode: mode.unwrap_or_default(),
            effort: Effort::Medium,
            saver,
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
            Msg::Reloaded { skills, summary } => {
                self.skills = skills;
                self.transcript.notice(summary);
            }
            Msg::Connected(provider) => {
                self.transcript.notice(format!("Connected {provider}."));
                self.load(false);
            }
            Msg::Clipboard(found) => self.clipboard(found),
            Msg::Changes(changes) => self.changes(changes),
            Msg::Diffs(scope, changes) => {
                if let Some(view) = &mut self.diffs {
                    view.loaded(scope, changes);
                }
            }
            Msg::Side { question, answer } => super::side::answered(self, &question, answer),
            Msg::Chore(answer) => self.chore_answered(answer),
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Error(text) => self.transcript.error(text),
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
        self.suggestion = 0;
        self.saver.remember_prompt(&self.cwd, &text);
        if let Some(command) = Command::parse(&text) {
            return super::commands::run(self, command);
        }
        if self.busy() {
            self.queued.push(text);
            return;
        }
        self.send(text);
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

    pub(super) fn spawn(&self, work: impl FnOnce(&Superfik) -> Msg + Send + 'static) {
        let (superfik, tx) = (self.superfik.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(work(&superfik));
        });
    }
}
