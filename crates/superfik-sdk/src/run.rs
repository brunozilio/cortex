//! One run of the agent and the handle that steers it from anywhere.

use std::sync::{Arc, Mutex, PoisonError};

use crossbeam_channel::Receiver;
use superfik_core::{AskId, Decision, EngineEvent, PermissionMode, RunId, SessionId};
use superfik_protocol::ClientCommand;

use crate::Error;
use crate::client::Client;
use crate::plan_watch::PlanWatch;

/// One run of the agent. Iterate it for its [`EngineEvent`]s; the last one
/// is `Completed`, `Failed` or `Cancelled`.
#[derive(Debug)]
pub struct Run {
    id: RunId,
    events: Receiver<EngineEvent>,
    client: Arc<Client>,
    /// Where a conversation's run records its session id.
    pub(crate) resume: Arc<Mutex<Option<SessionId>>>,
    /// A planning run's plan, read for its verification criteria.
    pub(crate) plan: Option<PlanWatch>,
    done: bool,
}

impl Run {
    /// Follows a new run, then sends the command that starts it.
    pub(crate) fn start(client: &Arc<Client>, command: impl FnOnce(RunId) -> ClientCommand) -> Result<Self, Error> {
        let run = RunId::new();
        let events = client.follow(run);
        if let Err(error) = client.send(command(run)) {
            client.forget(run);
            return Err(error);
        }
        Ok(Run { id: run, events, client: Arc::clone(client), resume: Arc::default(), plan: None, done: false })
    }

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
        if let Some(plan) = &mut self.plan {
            plan.observe(event);
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
