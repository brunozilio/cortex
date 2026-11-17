use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use crate::api::ClientCommand;
use domain::budget::Spending;
use domain::memory::MemoryBrief;
use domain::session_run::{Carried, RunWatch};
use domain::{Criteria, Effort, ModelOption, PermissionMode, PromptRequest, SessionId};

use crate::Error;
use crate::client::dispatch::Dispatcher;
use crate::client::run::Run;

#[derive(Debug)]
pub struct Session {
    dispatcher: Arc<Dispatcher>,
    cwd: PathBuf,
    model: ModelOption,
    effort: Effort,
    auto_pick: bool,
    memory: bool,
    carried: Arc<Mutex<Carried>>,
    added_dirs: Vec<PathBuf>,
    output_style: Option<String>,
    roles: domain::model_roles::ModelRoles,
    fallbacks: Vec<String>,
    headless: Headless,
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
    pub(crate) fn new(dispatcher: Arc<Dispatcher>, cwd: PathBuf, model: ModelOption) -> Self {
        Self {
            dispatcher,
            cwd,
            model,
            effort: Effort::Medium,
            auto_pick: false,
            memory: true,
            carried: Arc::default(),
            added_dirs: Vec::new(),
            output_style: None,
            roles: Default::default(),
            fallbacks: Vec::new(),
            headless: Headless::default(),
        }
    }

    pub fn learn(&self) -> Option<Result<Run, Error>> {
        let brief = self.carried.locked().lesson_due()?;
        let mut request = PromptRequest::plain(self.cwd.clone(), brief, self.model.clone());
        request.session.resume = Some(match self.id() {
            Some(id) => id,
            None => return Some(Err(Error::NothingToAsk)),
        });
        request.session.command = Some(domain::SessionCommand::Learn);
        Some(self.start(request))
    }

    pub fn set_budget(&self, limit_usd: Option<f64>) {
        self.carried.locked().spending.limit_usd = limit_usd;
    }

    pub fn spending(&self) -> Spending {
        self.carried.locked().spending
    }

    pub fn fork_next(&self) {
        self.carried.locked().fork_next();
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
        self.carried.locked().mode = mode;
        self
    }

    pub fn memory(mut self, on: bool) -> Self {
        self.memory = on;
        self
    }

    pub fn resume(self, session: SessionId) -> Self {
        self.carried.locked().id = Some(session);
        self
    }

    pub fn set_model(&mut self, model: ModelOption) {
        self.model = model;
    }

    pub fn set_roles(&mut self, roles: domain::model_roles::ModelRoles) {
        self.roles = roles;
    }

    pub fn set_fallbacks(&mut self, fallbacks: Vec<String>) {
        self.fallbacks = fallbacks;
    }

    pub fn set_permission(&mut self, mode: PermissionMode) {
        self.carried.locked().mode = mode;
    }

    pub fn set_effort(&mut self, effort: Effort) {
        self.effort = effort;
        self.auto_pick = false;
    }

    pub fn set_auto_pick(&mut self) {
        self.auto_pick = true;
    }

    pub fn effort_level(&self) -> Effort {
        self.effort
    }

    pub fn model(&self) -> &ModelOption {
        &self.model
    }

    pub fn permission_mode(&self) -> PermissionMode {
        self.carried.locked().mode
    }

    pub fn set_verification(&mut self, criteria: Option<String>) {
        self.carried.locked().verifications.set_user(criteria);
    }

    pub fn verification(&self) -> Option<Criteria> {
        self.carried.locked().verifications.current()
    }

    pub fn id(&self) -> Option<SessionId> {
        self.carried.locked().id.clone()
    }

    pub fn send(&self, prompt: &str) -> Result<Run, Error> {
        self.send_with_images(prompt, Vec::new())
    }

    pub fn send_with_images(&self, prompt: &str, images: Vec<crate::Image>) -> Result<Run, Error> {
        self.send_shown(prompt, images, None)
    }

    pub fn send_shown(
        &self,
        prompt: &str,
        images: Vec<crate::Image>,
        shown: Option<domain::request::Shown>,
    ) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), prompt, self.model.clone());
        request.prompt.images = images;
        request.prompt.shown = shown;
        request.session.effort = self.effort;
        request.session.auto_pick = self.auto_pick;
        request.host.memory = self.memory.then(|| MemoryBrief::new(None));
        {
            let mut carried = self.carried.locked();
            request.policy.permission = carried.mode;
            request.session.resume = carried.id.clone();
            request.session.fork = carried.forks();
            request.policy.verification = carried.criteria_for_run();
        }
        self.start(request)
    }

    pub fn measure_context(&self) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.session.resume = self.id();
        request.policy.permission = self.permission_mode();
        request.session.command = Some(domain::SessionCommand::MeasureContext);
        self.start(request)
    }

    pub fn compact(&self) -> Result<Run, Error> {
        self.compact_on("")
    }

    pub fn compact_on(&self, focus: &str) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), focus, self.model.clone());
        request.session.resume = Some(self.id().ok_or(Error::NothingToCompact)?);
        request.session.command = Some(domain::SessionCommand::Compact);
        self.start(request)
    }

    pub fn rewind(&self, index: u32) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.session.resume = Some(self.id().ok_or(Error::NothingToAsk)?);
        request.session.command = Some(domain::SessionCommand::Rewind(index));
        self.start(request)
    }

    pub fn branch(&self, index: u32) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.session.resume = Some(self.id().ok_or(Error::NothingToAsk)?);
        request.session.command = Some(domain::SessionCommand::Branch(index));
        self.start(request)
    }

    pub(crate) fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    pub(crate) fn start(&self, mut request: PromptRequest) -> Result<Run, Error> {
        request.session.add_dirs = self.added_dirs.clone();
        request.policy.max_turns = self.headless.max_turns;
        request.policy.allowed_tools.clone_from(&self.headless.allowed_tools);
        request.policy.disallowed_tools.clone_from(&self.headless.disallowed_tools);
        request.session.append_system.clone_from(&self.headless.append_system);
        request.session.fallback_model.clone_from(&self.headless.fallback_model);
        request.session.roles.clone_from(&self.roles);
        request.session.fallbacks.clone_from(&self.fallbacks);
        if request.session.output_style.is_none() {
            request.session.output_style.clone_from(&self.output_style);
        }
        request.policy.budget = self.spending().budget();
        let watch = RunWatch::of(request.session.command, request.policy.permission);
        let run = Run::start(&self.dispatcher, |run| ClientCommand::SubmitPrompt { run, request })?;
        Ok(run.of_session(Arc::clone(&self.carried), watch))
    }
}
