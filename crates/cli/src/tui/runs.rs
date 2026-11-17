use std::time::Instant;

use sdk::{EngineEvent, Run};

use super::app::{App, Msg};
use super::ask::Pending;
use super::stopwatch::Stopwatch;
use super::transcript::Entry;

impl App {
    pub(super) fn event(&mut self, run: domain::RunId, event: EngineEvent) {
        if self.turn.run.as_ref().is_none_or(|current| current.id() != run) {
            return;
        }
        match &event {
            EngineEvent::Asked(ask) => {
                self.ask = Some(Pending::new(ask.clone()));
                if let Some(watch) = &mut self.turn.started {
                    watch.pause(Instant::now());
                }
            }
            EngineEvent::AskWithdrawn(id) if self.ask.as_ref().is_some_and(|p| &p.ask.id == id) => self.ask_closed(),
            EngineEvent::SessionStarted(id) => self.saver.remember_session(&self.cwd, id),
            EngineEvent::Checkpoint(index) => self.checkpointed(*index),
            EngineEvent::Rewound(rewound) => self.turn.rewound = Some(rewound.clone()),
            EngineEvent::Branched { checkpoint, prompt, .. } => self.branched(*checkpoint, prompt),
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
            EngineEvent::Failure(kind) => self.turn.transient = *kind == domain::FailureKind::Unavailable,
            _ => {}
        }
        self.transcript.apply(&event);
        if let EngineEvent::Usage(usage) = &event {
            self.transcript.last_call = Some(domain::cache_window::LastCall::of(usage, super::conversations::now()));
        }
        if !self.focused && (matches!(event, EngineEvent::Asked(_)) || event.is_terminal()) {
            self.bell = true;
        }
        if event.is_terminal() {
            self.turn.run = None;
            self.ask = None;
            self.turn.started = None;
            if let Some(rewound) = self.turn.rewound.take() {
                self.rewound(rewound);
            } else if matches!(event, EngineEvent::Cancelled) {
                self.turn.retrying = false;
                self.unqueue();
            } else if matches!(event, EngineEvent::Failed(_)) && self.turn.queued.is_empty() {
                self.turn.retrying = false;
                self.transcript.notice("/retry sends it again from where it started.");
                super::auto_retry::failed(self);
            } else if !self.turn.queued.is_empty() {
                let next = self.turn.queued.remove(0);
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
        let prompt = super::mentions::with_files(&self.sdk, &self.cwd, &self.pastes.unfold(&text));
        let prompt = super::shell::with_context(&mut self.shell_context, &prompt);
        let started = session.send_with_images(&prompt, self.attachments.named_in(&text));
        self.transcript.clear_passing();
        self.transcript.push(Entry::User(shown));
        self.follow(started);
    }

    pub(super) fn take_back_queued(&mut self) {
        if let Some(run) = &self.turn.run {
            let _ = run.unsteer();
        }
    }

    fn dequeue(&mut self, text: &str) {
        if let Some(at) = self.turn.queued.iter().position(|queued| queued == text) {
            self.turn.queued.remove(at);
        }
    }

    fn unqueue(&mut self) {
        if self.turn.queued.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.turn.queued).join("\n\n");
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
                self.turn.run = Some(run.control());
                self.turn.started = Some(Stopwatch::start(Instant::now()));
                self.turn.transient = false;
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
        if let Some(run) = &self.turn.run
            && let Err(error) = run.cancel()
        {
            self.transcript.error(error.to_string());
        }
    }

    pub fn answer(&mut self, decision: sdk::Decision) {
        if super::git_offer::answered(self, &decision) || super::features::answered(self, &decision) {
            return;
        }
        let pending = self.ask.take();
        self.ask_closed();
        if let (Some(pending), Some(run)) = (pending, &self.turn.run)
            && let Err(error) = run.answer(pending.ask.id, decision)
        {
            self.transcript.error(error.to_string());
        }
    }

    fn ask_closed(&mut self) {
        self.ask = None;
        if let Some(watch) = &mut self.turn.started {
            watch.resume(Instant::now());
        }
    }
}
