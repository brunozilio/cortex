use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use sdk::{Effort, Model, PermissionMode, RunControl, Sdk, Session, SessionId};

use super::ask::Pending;
use super::choices::Picker;
use super::input::Input;
use super::login::SecretPrompt;
use super::transcript::Transcript;

pub use super::msg::Msg;

const QUIT_WINDOW: Duration = Duration::from_secs(2);

pub struct App {
    pub sdk: Sdk,
    pub cwd: PathBuf,
    pub session: Option<Session>,
    pub transcript: Transcript,
    pub input: Input,
    pub motion: std::cell::RefCell<super::motion::Motion>,
    pub attachments: super::attach::Attachments,
    pub pastes: super::pastes::Pastes,
    pub shell_context: Vec<String>,
    pub files: Option<Vec<String>>,
    pub show_tasks: bool,
    pub inline: Option<usize>,
    pub window: Option<u64>,
    pub conversations: Vec<super::conversations::Conversation>,
    pub status: super::statusline::Status,
    pub headless: sdk::Headless,
    pub fork_resumed: bool,
    pub output_style: Option<String>,
    pub connected: Vec<domain::provider::ProviderSummary>,
    pub menu: Vec<domain::provider::ProviderSummary>,
    pub vim: Option<super::vim::Vim>,
    pub mouse: bool,
    pub search: Option<super::search::Search>,
    pub external_edit: bool,
    pub edit_file: Option<PathBuf>,
    pub redraw: bool,
    pub expanded: bool,
    pub ask: Option<Pending>,
    pub secret: Option<SecretPrompt>,
    pub run: Option<RunControl>,
    pub started: Option<Instant>,
    pub queued: Vec<String>,
    pub notices: super::notices::Poll,
    #[allow(dead_code, reason = "the feature folders use it, and any of them may be removed")]
    pub features: super::features::States,
    pub added_dirs: Vec<PathBuf>,
    pub focused: bool,
    pub bell: bool,
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub picker: Option<Picker>,
    pub side: Option<super::side::Side>,
    pub title: Option<String>,
    pub ghost: Option<String>,
    pub checkpoints: Vec<super::rewind::Checkpoint>,
    pub(super) rewound: Option<domain::Rewound>,
    pub(super) retrying: bool,
    pub diffs: Option<super::diff_view::DiffView>,
    pub scroll: u16,
    pub quit: bool,
    pub quit_armed: Option<Instant>,
    pub clear_armed: Option<Instant>,
    pub shell: bool,
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
    pub fn new(sdk: Sdk, cwd: PathBuf, wanted: Option<String>, mode: Option<PermissionMode>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let saver = super::settings::Saver::start(&sdk);
        let mut app = Self {
            sdk,
            cwd,
            session: None,
            transcript: Transcript::default(),
            input: Input::default(),
            motion: std::cell::RefCell::new(super::motion::Motion::from_env()),
            attachments: Default::default(),
            pastes: Default::default(),
            shell_context: Vec::new(),
            files: None,
            show_tasks: true,
            inline: None,
            window: None,
            conversations: Vec::new(),
            connected: Vec::new(),
            menu: Vec::new(),
            output_style: None,
            headless: Default::default(),
            fork_resumed: false,
            status: Default::default(),
            vim: None,
            mouse: false,
            search: None,
            external_edit: false,
            edit_file: None,
            redraw: false,
            expanded: false,
            ask: None,
            secret: None,
            run: None,
            started: None,
            queued: Vec::new(),
            notices: Default::default(),
            features: Default::default(),
            added_dirs: Vec::new(),
            focused: true,
            bell: false,
            models: Vec::new(),
            skills: Vec::new(),
            picker: None,
            side: None,
            title: None,
            ghost: None,
            checkpoints: Vec::new(),
            rewound: None,
            retrying: false,
            diffs: None,
            scroll: 0,
            quit: false,
            quit_armed: None,
            clear_armed: None,
            shell: false,
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

    pub fn resuming(mut self, session: Option<SessionId>) -> Self {
        if session.is_some() {
            self.transcript.notice("Continuing the last conversation in this folder.");
        }
        self.resume = session;
        self
    }

    pub fn add_dir(&mut self, dir: PathBuf) {
        if let Some(session) = &mut self.session {
            session.add_dir(dir.clone());
        }
        if !self.added_dirs.contains(&dir) {
            self.added_dirs.push(dir);
        }
    }

    pub(super) fn load(&mut self, skills: bool) {
        self.loading = true;
        let (sdk, tx, cwd) = (self.sdk.clone(), self.tx.clone(), self.cwd.clone());
        std::thread::spawn(move || {
            let settings = sdk.settings().unwrap_or_else(|_| serde_json::json!({}));
            let models = sdk.models().map_err(|error| error.to_string());
            let _ = tx.send(Msg::Ready { models, settings });
            if skills && let Ok(found) = sdk.skills(&cwd) {
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

    pub fn quitting(&self) -> bool {
        self.quit_armed.is_some_and(|at| at.elapsed() < QUIT_WINDOW)
    }

    pub(super) fn spawn(&self, work: impl FnOnce(&Sdk) -> Msg + Send + 'static) {
        let (sdk, tx) = (self.sdk.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(work(&sdk));
        });
    }
}
