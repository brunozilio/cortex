//! A conversation with the agent: each [`Session::send`] starts a [`Run`]
//! that streams the agent's events; the next prompt resumes the same
//! conversation.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use crossbeam_channel::Receiver;
use superfik_core::memory::MemoryBrief;
use superfik_core::{
    AskId, Decision, Effort, EngineEvent, ModelOption, PermissionMode, PromptRequest, RunId, SessionId,
};
use superfik_protocol::ClientCommand;

use crate::Error;
use crate::client::Client;

#[derive(Debug)]
pub struct Session {
    client: Arc<Client>,
    cwd: PathBuf,
    model: ModelOption,
    effort: Effort,
    permission: PermissionMode,
    memory: bool,
    resume: Arc<Mutex<Option<SessionId>>>,
}

impl Session {
    pub(crate) fn new(client: Arc<Client>, cwd: PathBuf, model: ModelOption) -> Self {
        Self {
            client,
            cwd,
            model,
            effort: Effort::Medium,
            permission: PermissionMode::default(),
            memory: true,
            resume: Arc::default(),
        }
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = effort;
        self
    }

    /// How much the agent may do without asking (`Ask`, `AcceptEdits`,
    /// `Plan`, `Bypass`).
    pub fn permission(mut self, mode: PermissionMode) -> Self {
        self.permission = mode;
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
        self.permission = mode;
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
        self.permission
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
        request.permission = self.permission;
        request.resume = self.id();
        request.memory = self.memory.then(|| MemoryBrief::new(None));
        self.start(request)
    }

    /// Compacts the conversation so far into a summary.
    pub fn compact(&self) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd.clone(), "", self.model.clone());
        request.resume = Some(self.id().ok_or(Error::NothingToCompact)?);
        request.command = Some(superfik_core::SessionCommand::Compact);
        self.start(request)
    }

    fn start(&self, request: PromptRequest) -> Result<Run, Error> {
        let run = RunId::new();
        let events = self.client.follow(run);
        if let Err(error) = self.client.send(ClientCommand::SubmitPrompt { run, request }) {
            self.client.forget(run);
            return Err(error);
        }
        Ok(Run { id: run, events, client: Arc::clone(&self.client), resume: Arc::clone(&self.resume), done: false })
    }
}

/// One run of the agent. Iterate it for its [`EngineEvent`]s; the last one
/// is `Completed`, `Failed` or `Cancelled`.
#[derive(Debug)]
pub struct Run {
    id: RunId,
    events: Receiver<EngineEvent>,
    client: Arc<Client>,
    resume: Arc<Mutex<Option<SessionId>>>,
    done: bool,
}

impl Run {
    pub fn id(&self) -> RunId {
        self.id
    }

    /// Waits for the next event; `None` after the terminal one.
    pub fn next_event(&mut self) -> Option<EngineEvent> {
        if self.done {
            return None;
        }
        let event = self.events.recv().ok()?;
        self.observe(&event);
        Some(event)
    }

    /// The next event if one is waiting.
    pub fn try_next_event(&mut self) -> Option<EngineEvent> {
        if self.done {
            return None;
        }
        let event = self.events.try_recv().ok()?;
        self.observe(&event);
        Some(event)
    }

    fn observe(&mut self, event: &EngineEvent) {
        if let EngineEvent::SessionStarted(id) = event {
            *self.resume.lock().unwrap_or_else(PoisonError::into_inner) = Some(id.clone());
        }
        self.done = event.is_terminal();
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Answers a permission request or a question (`EngineEvent::Asked`).
    pub fn answer(&self, ask: AskId, decision: Decision) -> Result<(), Error> {
        self.control().answer(ask, decision)
    }

    pub fn cancel(&self) -> Result<(), Error> {
        self.control().cancel()
    }

    pub fn set_permission(&self, mode: PermissionMode) -> Result<(), Error> {
        self.control().set_permission(mode)
    }

    /// A handle that answers, cancels or changes this run's mode while
    /// another thread reads its events.
    pub fn control(&self) -> RunControl {
        RunControl { id: self.id, client: Arc::clone(&self.client) }
    }

    /// Runs to the end, denying anything the agent asks, and returns its
    /// text. For runs that need answers, iterate instead.
    pub fn text(mut self) -> Result<String, Error> {
        let mut text = String::new();
        while let Some(event) = self.next_event() {
            match event {
                EngineEvent::TextDelta(delta) => text.push_str(&delta),
                EngineEvent::Asked(ask) => self.answer(ask.id, Decision::Deny)?,
                EngineEvent::Failed(message) => return Err(Error::Run(message)),
                EngineEvent::Cancelled => return Err(Error::Run("cancelled".into())),
                _ => {}
            }
        }
        Ok(text)
    }
}

/// Steers a [`Run`] from anywhere; see [`Run::control`].
#[derive(Debug, Clone)]
pub struct RunControl {
    id: RunId,
    client: Arc<Client>,
}

impl RunControl {
    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn answer(&self, ask: AskId, decision: Decision) -> Result<(), Error> {
        self.client.send(ClientCommand::AnswerAsk { run: self.id, ask, decision })
    }

    pub fn cancel(&self) -> Result<(), Error> {
        self.client.send(ClientCommand::CancelRun { run: self.id })
    }

    pub fn set_permission(&self, mode: PermissionMode) -> Result<(), Error> {
        self.client.send(ClientCommand::SetPermissionMode { run: self.id, mode })
    }
}

impl Iterator for Run {
    type Item = EngineEvent;

    fn next(&mut self) -> Option<EngineEvent> {
        self.next_event()
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if !self.done {
            self.client.forget(self.id);
        }
    }
}
