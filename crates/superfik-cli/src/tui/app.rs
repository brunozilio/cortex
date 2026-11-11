//! The chat's state and what it does: send prompts (queued while the agent
//! works), run the chat's own `/commands`. Slow calls run on threads and
//! come back as [`Msg`]s. Following a run is in `runs`, choosing the model,
//! mode and effort in `choices`, connecting a provider in `login`, `/btw`
//! in `side`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use superfik_sdk::{Effort, Model, PermissionMode, RunControl, Session, SessionId, Superfik};

use super::ask::Pending;
use super::choices::Picker;
use super::input::Input;
use super::login::SecretPrompt;
use super::transcript::Transcript;

pub use super::msg::Msg;

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
    /// Long pastes folded in the prompt.
    pub pastes: super::pastes::Pastes,
    /// What `!commands` printed, for the next prompt.
    pub shell_context: Vec<String>,
    /// The project's files for `@` completion, once asked for.
    pub files: Option<Vec<String>>,
    /// Whether the agent's task list shows above the prompt (Ctrl+T).
    pub show_tasks: bool,
    /// Inline (`--inline`): how many entries went to the terminal's scrollback.
    pub inline: Option<usize>,
    /// The model's context window, once measured or known.
    pub window: Option<u64>,
    /// The conversations `/resume` offers, in the picker's order.
    pub conversations: Vec<super::conversations::Conversation>,
    /// The status line's command (`/statusline`), its output and last run.
    pub status: super::statusline::Status,
    /// The command line's rules and appended prompt, for every conversation.
    pub headless: superfik_sdk::Headless,
    /// The conversation resumed at launch goes on in a copy of itself.
    pub fork_resumed: bool,
    /// The output style picked with `/output-style`, kept between launches.
    pub output_style: Option<String>,
    /// The providers `/logout` offers, in the picker's order.
    pub connected: Vec<superfik_core::provider::ModelProvider>,
    /// Vim keys for the prompt (`/vim`).
    pub vim: Option<super::vim::Vim>,
    /// The wheel scrolls the conversation (`/mouse`); selecting needs Shift/Option.
    pub mouse: bool,
    /// Ctrl+R's search of the prompts sent before, while it runs.
    pub search: Option<super::search::Search>,
    /// Ctrl+G: edit the prompt in `$EDITOR` at the next frame.
    pub external_edit: bool,
    /// A file to open in `$EDITOR` at the next frame (`/memory edit`).
    pub edit_file: Option<PathBuf>,
    /// Ctrl+L: clear and redraw the screen at the next frame.
    pub redraw: bool,
    /// Ctrl+O: reasoning, outputs and diffs shown in full.
    pub expanded: bool,
    pub ask: Option<Pending>,
    pub secret: Option<SecretPrompt>,
    pub run: Option<RunControl>,
    pub started: Option<Instant>,
    pub queued: Vec<String>,
    /// Asking for scheduled prompts while idle.
    pub schedules: super::schedules::Poll,
    /// Folders read like the project (`/add-dir`, `--add-dir`), kept for every conversation.
    pub added_dirs: Vec<PathBuf>,
    /// Whether the terminal has focus (terminals that report it).
    pub focused: bool,
    /// Ring the bell at the next frame: the agent waits on an absent user.
    pub bell: bool,
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub picker: Option<Picker>,
    /// A `/btw` question and its answer, shown until dismissed.
    pub side: Option<super::side::Side>,
    /// The conversation's name, once a side call gave one.
    pub title: Option<String>,
    /// The prompt you would likely type next, shown in the empty prompt.
    pub ghost: Option<String>,
    /// This chat's prompts the engine can rewind to, oldest first.
    pub checkpoints: Vec<super::rewind::Checkpoint>,
    /// What the running rewind did, until it ends.
    pub(super) rewound: Option<superfik_core::Rewound>,
    /// Whether the running rewind sends its prompt again (`/retry`).
    pub(super) retrying: bool,
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
            pastes: Default::default(),
            shell_context: Vec::new(),
            files: None,
            show_tasks: true,
            inline: None,
            window: None,
            conversations: Vec::new(),
            connected: Vec::new(),
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
            schedules: Default::default(),
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

    /// Lets the agent read `dir` like the project, in this conversation and
    /// the next ones.
    pub fn add_dir(&mut self, dir: PathBuf) {
        if let Some(session) = &mut self.session {
            session.add_dir(dir.clone());
        }
        if !self.added_dirs.contains(&dir) {
            self.added_dirs.push(dir);
        }
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
