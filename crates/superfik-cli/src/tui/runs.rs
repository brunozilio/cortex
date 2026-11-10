//! Following the agent's runs: starting one, applying its events, sending
//! what was queued when it ends, and answering or cancelling it.

use std::time::Instant;

use superfik_sdk::{EngineEvent, Run};

use super::app::{App, Msg};
use super::ask::Pending;
use super::transcript::Entry;

impl App {
    pub(super) fn event(&mut self, run: superfik_core::RunId, event: EngineEvent) {
        if self.run.as_ref().is_none_or(|current| current.id() != run) {
            return;
        }
        match &event {
            EngineEvent::Asked(ask) => self.ask = Some(Pending::new(ask.clone())),
            EngineEvent::AskWithdrawn(id) if self.ask.as_ref().is_some_and(|p| &p.ask.id == id) => self.ask = None,
            EngineEvent::SessionStarted(id) => self.saver.remember_session(&self.cwd, id),
            EngineEvent::Checkpoint(index) => self.checkpointed(*index),
            EngineEvent::Rewound(rewound) => self.rewound = Some(rewound.clone()),
            _ => {}
        }
        self.transcript.apply(&event);
        if event.is_terminal() {
            self.run = None;
            self.ask = None;
            self.started = None;
            if let Some(rewound) = self.rewound.take() {
                self.rewound(rewound);
            } else if matches!(event, EngineEvent::Cancelled) {
                self.retrying = false;
                self.unqueue();
            } else if matches!(event, EngineEvent::Failed(_)) && self.queued.is_empty() {
                self.retrying = false;
                self.transcript.notice("/retry sends it again from where it started.");
            } else if !self.queued.is_empty() {
                let next = self.queued.remove(0);
                self.send(next);
            } else if matches!(event, EngineEvent::Completed { .. }) {
                self.after_run();
            }
        }
    }

    pub(super) fn send(&mut self, text: String) {
        self.ghost = None;
        let Some(session) = &self.session else {
            self.input.set(&text);
            return self.transcript.error("No model yet: /login connects a provider.");
        };
        let started = session.send_with_images(&text, self.attachments.named_in(&text));
        self.transcript.push(Entry::User(text));
        self.follow(started);
    }

    /// Puts what was queued back in the prompt, so an interrupt loses nothing.
    fn unqueue(&mut self) {
        if self.queued.is_empty() {
            return;
        }
        let mut text = std::mem::take(&mut self.queued).join("\n\n");
        if !self.input.is_empty() {
            text = format!("{text}\n\n{}", self.input.text());
        }
        self.input.set(&text);
    }

    pub(super) fn follow(&mut self, started: Result<Run, superfik_sdk::Error>) {
        match started {
            Ok(run) => {
                self.run = Some(run.control());
                self.started = Some(Instant::now());
                self.transcript.begin();
                self.scroll = 0;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let id = run.id();
                    for event in run {
                        if tx.send(Msg::Event(id, event)).is_err() {
                            break;
                        }
                    }
                });
            }
            Err(error) => self.transcript.error(error.to_string()),
        }
    }

    pub fn cancel(&mut self) {
        if let Some(run) = &self.run
            && let Err(error) = run.cancel()
        {
            self.transcript.error(error.to_string());
        }
    }

    pub fn answer(&mut self, decision: superfik_sdk::Decision) {
        if let (Some(pending), Some(run)) = (self.ask.take(), &self.run)
            && let Err(error) = run.answer(pending.ask.id, decision)
        {
            self.transcript.error(error.to_string());
        }
    }
}
