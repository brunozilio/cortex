use std::sync::{Arc, Mutex, PoisonError};

use crossbeam_channel::Receiver;
use domain::{AskId, Decision, EngineEvent, PermissionMode, RunId, SessionId};
use protocol::ClientCommand;

use crate::Error;
use crate::client::Client;
use crate::plan_watch::PlanWatch;

#[derive(Debug)]
pub struct Run {
    id: RunId,
    events: Receiver<EngineEvent>,
    client: Arc<Client>,
    pub(crate) resume: Arc<Mutex<Option<SessionId>>>,
    pub(crate) mode: Arc<Mutex<PermissionMode>>,
    pub(crate) plan: Option<PlanWatch>,
    done: bool,
}

impl Run {
    pub(crate) fn start(client: &Arc<Client>, command: impl FnOnce(RunId) -> ClientCommand) -> Result<Self, Error> {
        let run = RunId::new();
        let events = client.follow(run);
        if let Err(error) = client.send(command(run)) {
            client.forget(run);
            return Err(error);
        }
        Ok(Run {
            id: run,
            events,
            client: Arc::clone(client),
            resume: Arc::default(),
            mode: Arc::default(),
            plan: None,
            done: false,
        })
    }

    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn next_event(&mut self) -> Option<EngineEvent> {
        if self.done {
            return None;
        }
        let event = self.events.recv().ok()?;
        self.observe(&event);
        Some(event)
    }

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
        if let EngineEvent::ModeChanged(mode) = event {
            *self.mode.lock().unwrap_or_else(PoisonError::into_inner) = *mode;
        }
        if let Some(plan) = &mut self.plan {
            plan.observe(event);
        }
        self.done = event.is_terminal();
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn answer(&self, ask: AskId, decision: Decision) -> Result<(), Error> {
        self.control().answer(ask, decision)
    }

    pub fn cancel(&self) -> Result<(), Error> {
        self.control().cancel()
    }

    pub fn set_permission(&self, mode: PermissionMode) -> Result<(), Error> {
        self.control().set_permission(mode)
    }

    pub fn steer(&self, text: &str) -> Result<(), Error> {
        self.control().steer(text)
    }

    pub fn control(&self) -> RunControl {
        RunControl { id: self.id, client: Arc::clone(&self.client) }
    }

    pub fn text(mut self) -> Result<String, Error> {
        let mut text = String::new();
        while let Some(event) = self.next_event() {
            match event {
                EngineEvent::TextDelta(delta) => text.push_str(&delta),
                EngineEvent::Asked(ask) => {
                    if let domain::AskKind::Plan { plan } = &ask.kind {
                        text.push_str(plan);
                        text.push_str("\n\n");
                    }
                    self.answer(ask.id, Decision::Deny)?;
                }
                EngineEvent::Failed(message) => return Err(Error::Run(message)),
                EngineEvent::Cancelled => return Err(Error::Run("cancelled".into())),
                _ => {}
            }
        }
        Ok(text)
    }
}

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

    pub fn steer(&self, text: &str) -> Result<(), Error> {
        self.client.send(ClientCommand::SteerRun { run: self.id, text: text.to_owned() })
    }

    pub fn unsteer(&self) -> Result<(), Error> {
        self.client.send(ClientCommand::UnsteerRun { run: self.id })
    }

    pub fn background(&self) -> Result<(), Error> {
        self.client.send(ClientCommand::BackgroundRun { run: self.id })
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
