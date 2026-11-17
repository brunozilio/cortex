use crossbeam_channel::{Receiver, RecvTimeoutError};

use agent_engine::RunHandle;
use domain::{AgentAsk, Decision, EngineEvent, PromptRequest};

use crate::stage::PATIENCE;

pub struct Run {
    request: PromptRequest,
    events: Receiver<EngineEvent>,
    handle: Box<dyn RunHandle>,
    seen: Vec<EngineEvent>,
}

impl Run {
    pub fn open(request: &PromptRequest, events: Receiver<EngineEvent>, handle: Box<dyn RunHandle>) -> Self {
        Self { request: request.clone(), events, handle, seen: Vec::new() }
    }

    pub fn until(&mut self, wanted: impl Fn(&EngineEvent) -> bool) -> EngineEvent {
        loop {
            let event = self.events.recv_timeout(PATIENCE).unwrap_or_else(|_| {
                panic!("the run went quiet before the awaited event; seen so far: {:#?}", self.seen)
            });
            self.seen.push(event.clone());
            assert!(wanted(&event) || !event.is_terminal(), "the run ended before the awaited event: {:#?}", self.seen);
            if wanted(&event) {
                return event;
            }
        }
    }

    pub fn asked(&mut self) -> AgentAsk {
        match self.until(|event| matches!(event, EngineEvent::Asked(_))) {
            EngineEvent::Asked(ask) => ask,
            other => panic!("not an ask: {other:?}"),
        }
    }

    pub fn answer(&mut self, ask: &AgentAsk, decision: Decision) {
        self.handle.answer(&ask.id, &decision);
    }

    pub fn cancel(&mut self) {
        self.handle.cancel();
    }

    pub fn steer(&mut self, text: &str) {
        self.handle.steer(text.to_owned());
    }

    pub fn unsteer(&mut self) {
        self.handle.unsteer();
    }

    pub fn finish(mut self) -> (PromptRequest, Vec<EngineEvent>) {
        if !self.seen.last().is_some_and(EngineEvent::is_terminal) {
            self.until(EngineEvent::is_terminal);
        }
        drop(self.handle);
        loop {
            match self.events.recv_timeout(PATIENCE) {
                Ok(late) => self.seen.push(late),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => panic!("the run never let go of its event sink: {:#?}", self.seen),
            }
        }
        (self.request, self.seen)
    }
}
