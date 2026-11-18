use std::time::Instant;

use app::{EngineEvent, Run};

use super::app::{App, Msg};
use super::ask::Pending;
use super::stopwatch::Stopwatch;
use super::transcript::Entry;
use super::turn::Ticket;

pub struct Outgoing {
    prompt: String,
    images: Vec<app::Image>,
    shown: String,
}

impl App {
    pub(super) fn event(&mut self, run: domain::RunId, event: EngineEvent) {
        if self.turn.run.as_ref().is_none_or(|current| current.id() != run) {
            return;
        }
        self.before_shown(&event);
        self.transcript.apply(&event);
        if let EngineEvent::Usage(usage) = &event {
            self.transcript.last_call = Some(domain::cache_window::LastCall::of(usage, super::conversations::now()));
        }
        if !self.ui.focused && (matches!(event, EngineEvent::Asked(_)) || event.is_terminal()) {
            self.ui.bell = true;
        }
        if event.is_terminal() {
            self.run_ended(&event);
        }
    }

    fn before_shown(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::Asked(ask) => self.asked(ask),
            EngineEvent::AskWithdrawn(id) | EngineEvent::Answered { ask: id, .. }
                if self.ask.as_ref().is_some_and(|p| &p.ask.id == id) =>
            {
                self.ask_closed();
            }
            EngineEvent::SessionStarted(id) => self.saver.remember_session(&self.cwd, id),
            EngineEvent::Checkpoint(index) => self.checkpointed(*index),
            EngineEvent::Rewound(rewound) => self.turn.rewound = Some(rewound.clone()),
            EngineEvent::Branched { checkpoint, prompt, .. } => self.branched(*checkpoint, prompt),
            EngineEvent::Steered(text) => self.dequeue(text),
            EngineEvent::Unsteered(texts) => self.unsteered(texts),
            EngineEvent::ModeChanged(mode) => self.set_mode(*mode),
            EngineEvent::Failure(kind) => self.turn.transient = *kind == domain::FailureKind::Unavailable,
            _ => {}
        }
    }

    fn asked(&mut self, ask: &domain::AgentAsk) {
        if self.ask.replace(Pending::new(ask.clone())).is_some_and(|replaced| replaced.local) {
            super::git_offer::replaced(self);
        }
        if let Some(watch) = &mut self.turn.started {
            watch.pause(Instant::now());
        }
    }

    fn unsteered(&mut self, texts: &[String]) {
        for text in texts {
            self.dequeue(text);
        }
        if !texts.is_empty() {
            self.prompt.put_back(texts.join("\n\n"));
        }
    }

    fn run_ended(&mut self, event: &EngineEvent) {
        self.turn.run = None;
        let undelivered = std::mem::take(&mut self.turn.unresolved);
        self.turn.queued.extend(undelivered);
        self.ask.take_if(|pending| !pending.local);
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

    pub(super) fn send(&mut self, text: String) {
        self.send_as(text.clone(), text);
    }

    pub(super) fn send_as(&mut self, text: String, shown: String) {
        self.prompt.ghost = None;
        if self.session.is_none() {
            self.prompt.input.set(&text);
            return self.transcript.error("No model yet: /login connects a provider.");
        }
        if self.turn.preparing() {
            return self.turn.held.push(text);
        }
        let prompt = self.prompt.pastes.unfold(&text);
        let outgoing = Outgoing { prompt, images: self.prompt.attachments.named_in(&text), shown };
        if !super::mentions::names_a_file(&outgoing.prompt) {
            return self.start(outgoing);
        }
        let (ticket, cwd) = (self.turn.prepare(&text), self.cwd.clone());
        self.spawn(move |client| {
            let prompt = super::mentions::with_files(client, &cwd, &outgoing.prompt);
            Msg::Prepared { ticket, outgoing: Outgoing { prompt, ..outgoing } }
        });
    }

    pub(super) fn prepared(&mut self, ticket: Ticket, outgoing: Outgoing) {
        if self.turn.prepared(ticket) {
            self.start(outgoing);
            self.release_held();
        }
    }

    fn start(&mut self, outgoing: Outgoing) {
        let Some(session) = self.session.live() else { return };
        let prompt = super::typed_shell::with_context(&mut self.prompt.shell_context, &outgoing.prompt);
        let started = session.send_with_images(&prompt, outgoing.images);
        self.transcript.clear_passing();
        self.transcript.push(Entry::User(outgoing.shown));
        self.follow(started);
    }

    pub(super) fn release_held(&mut self) {
        if !self.busy() && !self.turn.held.is_empty() {
            let next = self.turn.held.remove(0);
            self.send(next);
        }
    }

    pub(super) fn steer(&mut self, text: &str) {
        self.turn.unresolved.push_back(self.prompt.pastes.unfold(text));
        if self.turn.unresolved.len() == 1 {
            self.resolve_steers();
        }
    }

    fn resolve_steers(&mut self) {
        while let Some(text) = self.turn.unresolved.front().cloned() {
            let run = self.turn.run.as_ref().map(app::RunControl::id);
            if let Some(run) = run.filter(|_| super::mentions::names_a_file(&text)) {
                let cwd = self.cwd.clone();
                return self.spawn(move |client| Msg::SteerResolved {
                    run,
                    text: super::mentions::with_files(client, &cwd, &text),
                });
            }
            self.turn.unresolved.pop_front();
            self.deliver_steer(text);
        }
    }

    pub(super) fn steer_resolved(&mut self, run: domain::RunId, text: String) {
        if self.turn.runs(run) && self.turn.unresolved.pop_front().is_some() {
            self.deliver_steer(text);
            self.resolve_steers();
        }
    }

    fn deliver_steer(&mut self, text: String) {
        if let Some(run) = &self.turn.run {
            let _ = run.steer(&text);
        }
        self.turn.queued.push(text);
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
        self.prompt.put_back(text);
    }

    pub(super) fn follow(&mut self, started: Result<Run, app::Error>) {
        match started {
            Ok(run) => {
                self.turn.run = Some(run.control());
                self.turn.started = Some(Stopwatch::start(Instant::now()));
                self.turn.transient = false;
                self.transcript.begin();
                self.ui.scroll = 0;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let id = run.id();
                    for event in run {
                        if tx.send(Msg::Event(id, event)).is_err() {
                            break;
                        }
                    }
                });
                for text in std::mem::take(&mut self.turn.held) {
                    self.steer(&text);
                }
            }
            Err(error) => self.transcript.error(error.to_string()),
        }
    }

    pub fn cancel(&mut self) {
        if let Some(run) = &self.turn.run {
            if let Err(error) = run.cancel() {
                self.transcript.error(error.to_string());
            }
        } else if let Some(typed) = self.turn.take_back() {
            super::git_offer::taken_back(self);
            self.prompt.put_back(typed);
        }
    }

    pub fn answer(&mut self, decision: app::Decision) {
        if super::git_offer::answered(self, &decision)
            || super::features::answered(self, &decision)
            || super::teammate_asks::answered(self, &decision)
        {
            return;
        }
        let pending = self.ask.take();
        self.ask_closed();
        if let (Some(pending), Some(run)) = (pending.filter(|pending| !pending.local), &self.turn.run)
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
        super::teammate_asks::show_next(self);
    }
}
