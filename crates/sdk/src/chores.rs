use domain::{ChoreAnswer, Decision, EngineEvent, PromptRequest, SessionCommand};

use crate::Error;
use crate::run::Run;
use crate::session::Session;

impl Session {
    pub fn side(&self, question: &str) -> Result<Run, Error> {
        self.chore(SessionCommand::Side, question)
    }

    pub fn title(&self, prompt: &str) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd().to_owned(), prompt, self.model().clone());
        request.command = Some(SessionCommand::Title);
        self.start(request)
    }

    pub fn suggest(&self) -> Result<Run, Error> {
        self.chore(SessionCommand::Suggest, "")
    }

    pub fn recap(&self) -> Result<Run, Error> {
        self.chore(SessionCommand::Recap, "")
    }

    pub fn heads_up(&self) -> Result<Run, Error> {
        self.chore(SessionCommand::HeadsUp, "")
    }

    fn chore(&self, command: SessionCommand, prompt: &str) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd().to_owned(), prompt, self.model().clone());
        request.resume = Some(self.id().ok_or(Error::NothingToAsk)?);
        request.command = Some(command);
        self.start(request)
    }
}

impl Run {
    pub fn side_answer(self) -> Result<String, Error> {
        match self.chore_answer()? {
            Some(ChoreAnswer::Side(text)) => Ok(text),
            _ => Err(Error::Run("the side question got no answer".into())),
        }
    }

    pub fn chore_answer(mut self) -> Result<Option<ChoreAnswer>, Error> {
        let mut answer = None;
        while let Some(event) = self.next_event() {
            match event {
                EngineEvent::Chore(given) => answer = Some(given),
                EngineEvent::Asked(ask) => self.answer(ask.id, Decision::Deny)?,
                EngineEvent::Failed(message) => return Err(Error::Run(message)),
                EngineEvent::Cancelled => return Err(Error::Run("cancelled".into())),
                _ => {}
            }
        }
        Ok(answer)
    }
}
