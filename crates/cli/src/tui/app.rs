use std::path::PathBuf;

use app::{Model, PermissionMode, Sdk, SessionId};
use crossbeam_channel::{Receiver, Sender};

use super::ask::Pending;
use super::catalog::Catalog;
use super::every::Every;
use super::overlays::Overlays;
use super::prompt_state::PromptState;
use super::session_state::SessionState;
use super::transcript::Transcript;
use super::turn::Turn;
use super::ui_state::Ui;
use super::workers::Workers;

pub use super::msg::Msg;

pub struct App {
    pub sdk: Sdk,
    pub cwd: PathBuf,
    pub session: SessionState,
    pub transcript: Transcript,
    pub prompt: PromptState,
    pub turn: Turn,
    pub ask: Option<Pending>,
    pub ui: Ui,
    pub overlays: Overlays,
    pub catalog: Catalog,
    pub fleet: super::fleet::Fleet,
    pub features: super::features::States,
    pub notices: Every,
    pub(super) tx: Sender<Msg>,
    workers: Workers,
    pub(super) saver: super::settings::Saver,
    pub rx: Receiver<Msg>,
}

impl App {
    pub fn new(sdk: Sdk, cwd: PathBuf, wanted: Option<String>, mode: Option<PermissionMode>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let saver = super::settings::Saver::start(&sdk);
        let mut app = Self {
            sdk,
            cwd,
            session: SessionState::new(wanted, mode),
            transcript: Transcript::default(),
            prompt: PromptState::default(),
            turn: Turn::default(),
            ask: None,
            ui: Ui::default(),
            overlays: Overlays::default(),
            catalog: Catalog { loading: true, ..Default::default() },
            fleet: Default::default(),
            features: Default::default(),
            notices: Every::second(),
            workers: Workers::default(),
            saver,
            tx,
            rx,
        };
        app.load(true);
        app
    }

    pub fn resuming(mut self, session: Option<SessionId>) -> Self {
        if session.is_some() {
            self.transcript.notice("Continuing the last conversation in this folder.");
        }
        self.session.resume = session;
        self
    }

    pub(super) fn load(&mut self, skills: bool) {
        self.catalog.loading = true;
        let (sdk, tx, cwd) = (self.sdk.clone(), self.tx.clone(), self.cwd.clone());
        std::thread::spawn(move || {
            if let Err(error) = sdk.register_peer(&cwd) {
                tracing::debug!(%error, "other sessions cannot reach this one yet");
            }
            let read = sdk.settings().map_err(|error| error.to_string());
            let (settings, unreadable) = super::settings::or_reported(read);
            if let Some(told) = unreadable {
                let _ = tx.send(Msg::Error(told));
            }
            let models = sdk.models().map_err(|error| error.to_string());
            let _ = tx.send(Msg::Ready { models, settings });
            if skills && let Ok(found) = sdk.skills(&cwd) {
                let _ = tx.send(Msg::Skills(found.into_iter().map(|s| (s.name, s.description)).collect()));
            }
        });
    }

    pub fn busy(&self) -> bool {
        self.turn.busy()
    }

    pub fn mode(&self) -> PermissionMode {
        self.session.mode()
    }

    pub fn effort_label(&self) -> &'static str {
        self.session.effort_label()
    }

    pub fn model(&self) -> Option<&Model> {
        self.session.model()
    }

    pub fn interrupt(&mut self) {
        if self.busy() {
            self.cancel();
        } else if !self.prompt.input.is_empty() {
            self.prompt.input.clear();
        } else {
            self.ui.quit_or_arm();
        }
    }

    pub fn quitting(&self) -> bool {
        self.ui.quitting()
    }

    pub(super) fn spawn(&self, work: impl FnOnce(&Sdk) -> Msg + Send + 'static) {
        let (sdk, tx) = (self.sdk.clone(), self.tx.clone());
        self.workers.run(move || {
            let _ = tx.send(work(&sdk));
        });
    }
}
