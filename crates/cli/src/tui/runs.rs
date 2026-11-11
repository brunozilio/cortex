use std::time::Instant;

use sdk::{EngineEvent, Run};

use super::app::{App, Msg};
use super::ask::Pending;
use super::transcript::Entry;

impl App {
    pub(super) fn event(&mut self, run: domain::RunId, event: EngineEvent) {
        if self.run.as_ref().is_none_or(|current| current.id() != run) {
            return;
        }
        match &event {
            EngineEvent::Asked(ask) => self.ask = Some(Pending::new(ask.clone())),
            EngineEvent::AskWithdrawn(id) if self.ask.as_ref().is_some_and(|p| &p.ask.id == id) => self.ask = None,
            EngineEvent::SessionStarted(id) => self.saver.remember_session(&self.cwd, id),
            EngineEvent::Checkpoint(index) => self.checkpointed(*index),
            EngineEvent::Rewound(rewound) => self.rewound = Some(rewound.clone()),
            EngineEvent::Steered(text) => {
                self.dequeue(text);
            }
            EngineEvent::Unsteered(texts) => {
                for text in texts {
                    self.dequeue(text);
                }
                if !texts.is_empty() {
                    self.back_in_prompt(texts.join("\n\n"));
                }
            }
            EngineEvent::ModeChanged(mode) => self.set_mode(*mode),
            _ => {}
        }
        self.transcript.apply(&event);
        if !self.focused && (matches!(event, EngineEvent::Asked(_)) || event.is_terminal()) {
            self.bell = true;
        }
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
        self.send_as(text.clone(), text);
    }

    pub(super) fn send_as(&mut self, text: String, shown: String) {
        self.ghost = None;
        let Some(session) = &self.session else {
            self.input.set(&text);
            return self.transcript.error("No model yet: /login connects a provider.");
        };
        let prompt = super::mentions::with_files(&self.cwd, &self.pastes.unfold(&text));
        let prompt = super::shell::with_context(&mut self.shell_context, &prompt);
        let started = session.send_with_images(&prompt, self.attachments.named_in(&text));
        self.transcript.push(Entry::User(shown));
        self.follow(started);
    }

    pub(super) fn take_back_queued(&mut self) {
        if let Some(run) = &self.run {
            let _ = run.unsteer();
        }
    }

    fn dequeue(&mut self, text: &str) {
        if let Some(at) = self.queued.iter().position(|queued| queued == text) {
            self.queued.remove(at);
        }
    }

    fn unqueue(&mut self) {
        if self.queued.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.queued).join("\n\n");
        self.back_in_prompt(text);
    }

    fn back_in_prompt(&mut self, mut text: String) {
        if !self.input.is_empty() {
            text = format!("{text}\n\n{}", self.input.text());
        }
        self.input.set(&text);
    }

    pub(super) fn follow(&mut self, started: Result<Run, sdk::Error>) {
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

    pub fn answer(&mut self, decision: sdk::Decision) {
        if let (Some(pending), Some(run)) = (self.ask.take(), &self.run)
            && let Err(error) = run.answer(pending.ask.id, decision)
        {
            self.transcript.error(error.to_string());
        }
    }
}
