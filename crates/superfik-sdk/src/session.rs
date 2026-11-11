//! A conversation with the agent: each [`Session::send`] starts a [`Run`]
//! that streams the agent's events; the next prompt resumes the same
//! conversation.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use superfik_core::memory::MemoryBrief;
use superfik_core::{Criteria, Effort, ModelOption, PermissionMode, PromptRequest, SessionId, Verifications};
use superfik_protocol::ClientCommand;

use crate::Error;
use crate::client::Client;
use crate::plan_watch::PlanWatch;
use crate::run::Run;

#[derive(Debug)]
pub struct Session {
    client: Arc<Client>,
    cwd: PathBuf,
    model: ModelOption,
    effort: Effort,
    /// Shared with each run, which follows the agent leaving or entering plan mode.
    permission: Arc<Mutex<PermissionMode>>,
    memory: bool,
    resume: Arc<Mutex<Option<SessionId>>>,
    verifications: Arc<Mutex<Verifications>>,
    added_dirs: Vec<PathBuf>,
    output_style: Option<String>,
    headless: Headless,
    /// The next prompt forks the conversation (`fork_next`).
    fork: std::sync::atomic::AtomicBool,
}

/// What a scripted session sets for every run (`superfik -p`'s flags).
#[derive(Debug, Clone, Default)]
pub struct Headless {
    /// The most model turns a run may take.
    pub max_turns: Option<u32>,
    /// Permission rules on top of the settings': allowed, then denied.
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    /// Text added to the end of the system prompt.
    pub append_system: Option<String>,
    /// The model to go on with when the run's is overloaded.
    pub fallback_model: Option<String>,
}

impl Session {
    pub(crate) fn new(client: Arc<Client>, cwd: PathBuf, model: ModelOption) -> Self {
        Self {
            client,
            cwd,
            model,
            effort: Effort::Medium,
            permission: Arc::default(),
            memory: true,
            resume: Arc::default(),
            verifications: Arc::default(),
            added_dirs: Vec::new(),
            output_style: None,
            headless: Headless::default(),
            fork: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The next prompt goes on from this conversation under a new session,
    /// leaving this one as it is (`--fork-session`, `/fork`).
    pub fn fork_next(&self) {
        self.fork.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Turn limit, rules and appended prompt for every run from now on.
    pub fn set_headless(&mut self, headless: Headless) {
        self.headless = headless;
    }

    /// How the agent talks from the next run on: a built-in style's id
    /// (`concise`, `proactive`, `explanatory`, `learning`), `default` for
    /// none, or `None` for the settings' `outputStyle`.
    pub fn set_output_style(&mut self, style: Option<String>) {
        self.output_style = style;
    }

    /// Lets the agent read folder `dir` like the project's own, from the
    /// next run on (absolute, or from the project).
    pub fn add_dir(&mut self, dir: PathBuf) {
        if !self.added_dirs.contains(&dir) {
            self.added_dirs.push(dir);
        }
    }

    /// The folders [`add_dir`](Self::add_dir) added.
    pub fn added_dirs(&self) -> &[PathBuf] {
        &self.added_dirs
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = effort;
        self
    }

    /// How much the agent may do without asking (`Ask`, `AcceptEdits`,
    /// `Plan`, `Bypass`).
    pub fn permission(self, mode: PermissionMode) -> Self {
        *self.permission.lock().unwrap_or_else(PoisonError::into_inner) = mode;
        self
    }

    /// Whether the agent recalls and learns project memory (on by default).
    pub fn memory(mut self, on: bool) -> Self {
        self.memory = on;
        self
    }

    /// Continues an earlier conversation.
    pub fn resume(self, session: SessionId) -> Self {
        *self.resume.lock().unwrap_or_else(PoisonError::into_inner) = Some(session);
        self
    }

    pub fn set_model(&mut self, model: ModelOption) {
        self.model = model;
    }

    pub fn set_permission(&mut self, mode: PermissionMode) {
        *self.permission.lock().unwrap_or_else(PoisonError::into_inner) = mode;
    }

    pub fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
    }

    pub fn effort_level(&self) -> Effort {
        self.effort
    }

    pub fn model(&self) -> &ModelOption {
        &self.model
    }

    pub fn permission_mode(&self) -> PermissionMode {
        *self.permission.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Judges every run outside planning against `criteria` before it
    /// completes (`None` or blank clears them). Without them, the last
    /// plan's `Verification` section is used.
    pub fn set_verification(&mut self, criteria: Option<String>) {
        self.verifications.lock().unwrap_or_else(PoisonError::into_inner).set_user(criteria);
    }

    /// The criteria the next run is judged against: the user's, else the
    /// last plan's.
    pub fn verification(&self) -> Option<Criteria> {
        self.verifications.lock().unwrap_or_else(PoisonError::into_inner).current()
    }

    /// The engine's id of this conversation, once its first run started.
    pub fn id(&self) -> Option<SessionId> {
        self.resume.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Sends a prompt (or `/<command> <arguments>`) and starts a run.
    pub fn send(&self, prompt: &str) -> Result<Run, Error> {
        self.send_with_images(prompt, Vec::new())
    }

    /// Sends a prompt with images the model sees before its text
    /// ([`Image::from_bytes`](crate::Image::from_bytes) reads one).
    pub fn send_with_images(&self, prompt: &str, images: Vec<crate::Image>) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), prompt, self.model.clone());
        request.images = images;
        request.effort = self.effort;
        let mode = self.permission_mode();
        request.permission = mode;
        request.resume = self.id();
        request.fork = request.resume.is_some() && self.fork.swap(false, std::sync::atomic::Ordering::SeqCst);
        request.memory = self.memory.then(|| MemoryBrief::new(None));
        request.verification = self.verifications.lock().unwrap_or_else(PoisonError::into_inner).for_run(mode);
        self.start(request)
    }

    /// Measures what fills the context window now (system prompt, tools,
    /// messages…), answered with [`EngineEvent::ContextMeasured`]; no model
    /// call is made.
    pub fn measure_context(&self) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.resume = self.id();
        request.permission = self.permission_mode();
        request.command = Some(superfik_core::SessionCommand::MeasureContext);
        self.start(request)
    }

    /// Compacts the conversation so far into a summary.
    pub fn compact(&self) -> Result<Run, Error> {
        self.compact_on("")
    }

    /// [`compact`](Self::compact), the summary keeping what `focus` names
    /// ("the API changes and the failing test").
    pub fn compact_on(&self, focus: &str) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), focus, self.model.clone());
        request.resume = Some(self.id().ok_or(Error::NothingToCompact)?);
        request.command = Some(superfik_core::SessionCommand::Compact);
        self.start(request)
    }

    /// Goes back to checkpoint `index` (each prompt's run announces its
    /// number with [`EngineEvent::Checkpoint`]): the files the agent edited
    /// since are restored and the conversation is cut before that prompt.
    /// The run ends with [`EngineEvent::Rewound`].
    pub fn rewind(&self, index: u32) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.resume = Some(self.id().ok_or(Error::NothingToAsk)?);
        request.command = Some(superfik_core::SessionCommand::Rewind(index));
        self.start(request)
    }

    pub(crate) fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    pub(crate) fn start(&self, mut request: PromptRequest) -> Result<Run, Error> {
        request.add_dirs = self.added_dirs.clone();
        request.max_turns = self.headless.max_turns;
        request.allowed_tools.clone_from(&self.headless.allowed_tools);
        request.disallowed_tools.clone_from(&self.headless.disallowed_tools);
        request.append_system.clone_from(&self.headless.append_system);
        request.fallback_model.clone_from(&self.headless.fallback_model);
        if request.output_style.is_none() {
            request.output_style.clone_from(&self.output_style);
        }
        // Only a run of the conversation itself can name its session; a
        // side call about it never does.
        let side_call = request.command.is_some_and(superfik_core::SessionCommand::is_chore);
        let resume = if side_call { Arc::default() } else { Arc::clone(&self.resume) };
        let conversation = request.command.is_none();
        let planning = request.permission == PermissionMode::Plan;
        let plan = conversation.then(|| PlanWatch::new(Arc::clone(&self.verifications), planning));
        let mut run = Run::start(&self.client, |run| ClientCommand::SubmitPrompt { run, request })?;
        run.resume = resume;
        run.plan = plan;
        if conversation {
            run.mode = Arc::clone(&self.permission);
        }
        Ok(run)
    }
}
