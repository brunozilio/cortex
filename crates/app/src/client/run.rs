use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use crate::api::{ClientCommand, Reply};
use crossbeam_channel::Receiver;
use domain::session_run::{Carried, RunWatch};
use domain::{AskId, Decision, EngineEvent, PermissionMode, RunId};

use crate::Error;
use crate::client::dispatch::{Dispatcher, Pending};

const BACKEND_GONE: &str = "the backend went away before the run ended";

#[derive(Debug)]
pub struct Run {
    id: RunId,
    events: Receiver<EngineEvent>,
    accepted: Option<Pending>,
    left_running: bool,
    dispatcher: Arc<Dispatcher>,
    carried: Arc<Mutex<Carried>>,
    watch: RunWatch,
    done: bool,
}

impl Run {
    pub(crate) fn start(
        dispatcher: &Arc<Dispatcher>,
        command: impl FnOnce(RunId) -> ClientCommand,
    ) -> Result<Self, Error> {
        let run = RunId::new();
        let events = dispatcher.follow(run);
        let accepted = match dispatcher.ask(command(run)) {
            Ok(accepted) => accepted,
            Err(error) => {
                dispatcher.forget(run);
                return Err(error);
            }
        };
        Ok(Run {
            id: run,
            events,
            accepted: Some(accepted),
            left_running: false,
            dispatcher: Arc::clone(dispatcher),
            carried: Arc::default(),
            watch: RunWatch::default(),
            done: false,
        })
    }

    pub(crate) fn of_session(mut self, carried: Arc<Mutex<Carried>>, watch: RunWatch) -> Self {
        (self.carried, self.watch) = (carried, watch);
        self
    }

    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn next_event(&mut self) -> Option<EngineEvent> {
        if self.done {
            return None;
        }
        let event = self.awaited();
        self.observe(&event);
        Some(event)
    }

    pub fn try_next_event(&mut self) -> Option<EngineEvent> {
        if self.done {
            return None;
        }
        let event = match self.refused_already() {
            Some(refused) => refused,
            None => match self.events.try_recv() {
                Ok(event) => event,
                Err(crossbeam_channel::TryRecvError::Empty) => return None,
                Err(crossbeam_channel::TryRecvError::Disconnected) => gone(),
            },
        };
        self.observe(&event);
        Some(event)
    }

    pub fn leave_running(mut self) {
        self.left_running = true;
    }

    fn awaited(&mut self) -> EngineEvent {
        loop {
            let Some(accepted) = self.accepted.as_ref().map(|pending| pending.replies().clone()) else {
                return self.events.recv().unwrap_or_else(|_| gone());
            };
            crossbeam_channel::select! {
                recv(self.events) -> event => return event.unwrap_or_else(|_| gone()),
                recv(accepted) -> reply => {
                    self.accepted = None;
                    if let Some(message) = reply.ok().and_then(refusal) {
                        return EngineEvent::Failed(message);
                    }
                }
            }
        }
    }

    fn refused_already(&mut self) -> Option<EngineEvent> {
        let reply = match self.accepted.as_ref()?.replies().try_recv() {
            Ok(reply) => Some(reply),
            Err(crossbeam_channel::TryRecvError::Empty) => return None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => None,
        };
        self.accepted = None;
        reply.and_then(refusal).map(EngineEvent::Failed)
    }

    fn observe(&mut self, event: &EngineEvent) {
        self.carried.locked().observe(&mut self.watch, event);
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
        RunControl { id: self.id, dispatcher: Arc::clone(&self.dispatcher) }
    }

    pub fn text(mut self) -> Result<String, Error> {
        let mut text = String::new();
        let mut kind = domain::FailureKind::Other;
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
                EngineEvent::Failure(given) => kind = given,
                EngineEvent::Failed(message) => return Err(domain::failure::RunFailure { kind, message }.into()),
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
    dispatcher: Arc<Dispatcher>,
}

impl RunControl {
    pub fn id(&self) -> RunId {
        self.id
    }

    pub fn answer(&self, ask: AskId, decision: Decision) -> Result<(), Error> {
        self.dispatcher.send(ClientCommand::AnswerAsk { run: self.id, ask, decision })
    }

    pub fn cancel(&self) -> Result<(), Error> {
        self.dispatcher.send(ClientCommand::CancelRun { run: self.id })
    }

    pub fn set_permission(&self, mode: PermissionMode) -> Result<(), Error> {
        self.dispatcher.send(ClientCommand::SetPermissionMode { run: self.id, mode })
    }

    pub fn steer(&self, text: &str) -> Result<(), Error> {
        self.dispatcher.send(ClientCommand::SteerRun { run: self.id, text: text.to_owned() })
    }

    pub fn unsteer(&self) -> Result<(), Error> {
        self.dispatcher.send(ClientCommand::UnsteerRun { run: self.id })
    }

    pub fn background(&self) -> Result<(), Error> {
        self.dispatcher.send(ClientCommand::BackgroundRun { run: self.id })
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
        if self.done {
            return;
        }
        self.dispatcher.forget(self.id);
        if !self.left_running {
            let _ = self.dispatcher.send(ClientCommand::CancelRun { run: self.id });
        }
    }
}

fn gone() -> EngineEvent {
    EngineEvent::Failed(BACKEND_GONE.into())
}

fn refusal(reply: Reply) -> Option<String> {
    match reply {
        Reply::Error { message } => Some(message),
        Reply::FeatureFailed(error) => Some(error.to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
