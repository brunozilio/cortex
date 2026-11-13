use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use domain::budget::Budget;
use domain::memory::MemoryBrief;
use domain::{Criteria, Effort, ModelOption, PermissionMode, PromptRequest, SessionId, Verifications};
use protocol::ClientCommand;

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
    permission: Arc<Mutex<PermissionMode>>,
    memory: bool,
    resume: Arc<Mutex<Option<SessionId>>>,
    verifications: Arc<Mutex<Verifications>>,
    added_dirs: Vec<PathBuf>,
    output_style: Option<String>,
    headless: Headless,
    fork: std::sync::atomic::AtomicBool,
    spending: Arc<Mutex<Spending>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spending {
    pub limit_usd: Option<f64>,
    pub spent_usd: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Headless {
    pub max_turns: Option<u32>,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub append_system: Option<String>,
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
            spending: Arc::default(),
        }
    }

    pub fn set_budget(&self, limit_usd: Option<f64>) {
        self.spending.locked().limit_usd = limit_usd;
    }

    pub fn spending(&self) -> Spending {
        *self.spending.locked()
    }

    pub fn fork_next(&self) {
        self.fork.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn set_headless(&mut self, headless: Headless) {
        self.headless = headless;
    }

    pub fn set_output_style(&mut self, style: Option<String>) {
        self.output_style = style;
    }

    pub fn add_dir(&mut self, dir: PathBuf) {
        if !self.added_dirs.contains(&dir) {
            self.added_dirs.push(dir);
        }
    }

    pub fn added_dirs(&self) -> &[PathBuf] {
        &self.added_dirs
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = effort;
        self
    }

    pub fn permission(self, mode: PermissionMode) -> Self {
        *self.permission.locked() = mode;
        self
    }

    pub fn memory(mut self, on: bool) -> Self {
        self.memory = on;
        self
    }

    pub fn resume(self, session: SessionId) -> Self {
        *self.resume.locked() = Some(session);
        self
    }

    pub fn set_model(&mut self, model: ModelOption) {
        self.model = model;
    }

    pub fn set_permission(&mut self, mode: PermissionMode) {
        *self.permission.locked() = mode;
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
        *self.permission.locked()
    }

    pub fn set_verification(&mut self, criteria: Option<String>) {
        self.verifications.locked().set_user(criteria);
    }

    pub fn verification(&self) -> Option<Criteria> {
        self.verifications.locked().current()
    }

    pub fn id(&self) -> Option<SessionId> {
        self.resume.locked().clone()
    }

    pub fn send(&self, prompt: &str) -> Result<Run, Error> {
        self.send_with_images(prompt, Vec::new())
    }

    pub fn send_with_images(&self, prompt: &str, images: Vec<crate::Image>) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), prompt, self.model.clone());
        request.images = images;
        request.effort = self.effort;
        let mode = self.permission_mode();
        request.permission = mode;
        request.resume = self.id();
        request.fork = request.resume.is_some() && self.fork.swap(false, std::sync::atomic::Ordering::SeqCst);
        request.memory = self.memory.then(|| MemoryBrief::new(None));
        request.verification = self.verifications.locked().for_run(mode);
        self.start(request)
    }

    pub fn measure_context(&self) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.resume = self.id();
        request.permission = self.permission_mode();
        request.command = Some(domain::SessionCommand::MeasureContext);
        self.start(request)
    }

    pub fn compact(&self) -> Result<Run, Error> {
        self.compact_on("")
    }

    pub fn compact_on(&self, focus: &str) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), focus, self.model.clone());
        request.resume = Some(self.id().ok_or(Error::NothingToCompact)?);
        request.command = Some(domain::SessionCommand::Compact);
        self.start(request)
    }

    pub fn rewind(&self, index: u32) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.resume = Some(self.id().ok_or(Error::NothingToAsk)?);
        request.command = Some(domain::SessionCommand::Rewind(index));
        self.start(request)
    }

    pub fn branch(&self, index: u32) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.resume = Some(self.id().ok_or(Error::NothingToAsk)?);
        request.command = Some(domain::SessionCommand::Branch(index));
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
        let side_call = request.command.is_some_and(domain::SessionCommand::is_chore);
        let resume = if side_call { Arc::default() } else { Arc::clone(&self.resume) };
        let conversation = request.command.is_none();
        let planning = request.permission == PermissionMode::Plan;
        let plan = conversation.then(|| PlanWatch::new(Arc::clone(&self.verifications), planning));
        let spending = self.spending();
        request.budget = spending.limit_usd.map(|limit_usd| Budget { limit_usd, spent_usd: spending.spent_usd });
        let mut run = Run::start(&self.client, |run| ClientCommand::SubmitPrompt { run, request })?;
        run.spending = Arc::clone(&self.spending);
        run.resume = resume;
        run.plan = plan;
        if conversation {
            run.mode = Arc::clone(&self.permission);
        }
        Ok(run)
    }
}
