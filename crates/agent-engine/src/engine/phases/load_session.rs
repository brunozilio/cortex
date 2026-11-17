use std::sync::Arc;

use domain::{EngineEvent, EngineProblem, SessionCommand, SessionId};

use crate::engine::MAX_OUTPUT;
use crate::engine::job::Job;
use crate::engine::stop::Stop;
use crate::meter::{Ledger, Metered};
use crate::model::TurnRequest;
use crate::transcript::Message;

pub(in crate::engine) struct LoadedSession {
    pub(in crate::engine) id: SessionId,
    pub(in crate::engine) history: Vec<Message>,
}

impl Job {
    pub(in crate::engine) fn load_session(&mut self) -> Result<LoadedSession, Stop> {
        let command = self.request.session.command;
        if command.is_some_and(|c| c != SessionCommand::Title) && self.request.session.resume.is_none() {
            return Err(Stop::failed("There is no conversation to work on yet."));
        }
        self.hold_the_answered_session()?;
        let stored = self.stored_or_new()?;
        let forked = self.request.session.fork && command.is_none();
        let session = if forked { LoadedSession { id: new_id(), ..stored } } else { stored };
        match command {
            Some(chore) if chore.is_chore() => Err(self.chore(chore, &session.history)),
            Some(SessionCommand::Rewind(index)) => Err(self.rewind(session, index)),
            Some(SessionCommand::Branch(index)) => Err(self.branch(&session, index)),
            _ => {
                self.open_session(&session.id);
                Ok(session)
            }
        }
    }

    fn hold_the_answered_session(&mut self) -> Result<(), Stop> {
        let answering = self
            .request
            .session
            .resume
            .as_ref()
            .filter(|_| !self.request.session.command.is_some_and(|c| c.is_chore()));
        let held =
            answering.map(|id| self.sessions.hold(&self.request.session.cwd, id)).transpose().map_err(Stop::failed)?;
        self.hold(held);
        Ok(())
    }

    fn stored_or_new(&self) -> Result<LoadedSession, Stop> {
        let Some(id) = &self.request.session.resume else {
            return Ok(LoadedSession { id: new_id(), history: Vec::new() });
        };
        let loaded = self.sessions.load_for_checked(&self.request.session.cwd, id);
        for notice in self.sessions.take_notices() {
            self.gate.emit(EngineEvent::Notice(notice));
        }
        let history = match loaded {
            Ok(Some(history)) => {
                let mut history = crate::sessions::repaired(history);
                crate::upkeep::forget_lost_transcripts(&mut history);
                history
            }
            Err(error) => return Err(Stop::failed(format!("Could not restore the conversation: {error}"))),
            Ok(None) if self.request.session.command.is_some_and(SessionCommand::is_chore) => Vec::new(),
            Ok(None) => {
                return Err(Stop::Diagnosed {
                    problem: EngineProblem::SessionNotFound,
                    message: "The conversation to continue was not found.".into(),
                });
            }
        };
        Ok(LoadedSession { id: id.clone(), history })
    }

    fn chore(&self, command: SessionCommand, history: &[Message]) -> Stop {
        let api = match self.source.api(&self.request) {
            Ok(api) => api,
            Err(error) => return Stop::Model(error),
        };
        let ledger = Arc::new(Ledger::default());
        let api =
            Metered { calls: None, budget: None, ..self.metered(api, &ledger, self.request.session.model.clone()) };
        let template = TurnRequest {
            model: self.request.session.model.id.clone(),
            system: crate::prompt::text("chores"),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: MAX_OUTPUT,
        };
        let stop = self.control.stop_flag();
        match crate::chores::answer(command, &api, &template, &self.request.prompt.text, history, stop) {
            Ok(Some(answer)) => {
                self.gate.emit(EngineEvent::Chore(answer));
            }
            Ok(None) => {}
            Err(error) => return Stop::Model(error),
        }
        Stop::completed(ledger.cost())
    }
}

fn new_id() -> SessionId {
    SessionId(uuid::Uuid::new_v4().to_string())
}
