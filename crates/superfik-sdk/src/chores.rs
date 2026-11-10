//! Side calls about a session: one model turn without tools, beside or
//! after its runs, never added to the conversation. A title for its first
//! prompt, the prompt the user would likely type next, a recap for someone
//! coming back, a side question, and the rare heads-up worth knowing.

use superfik_core::{ChoreAnswer, Decision, EngineEvent, PromptRequest, SessionCommand};

use crate::Error;
use crate::run::Run;
use crate::session::Session;

impl Session {
    /// Asks a side question about the conversation so far: answered without
    /// tools and not added to the conversation, so it can run beside a run
    /// in progress (which it sees up to its last finished turn).
    /// [`Run::side_answer`] waits for the answer.
    pub fn side(&self, question: &str) -> Result<Run, Error> {
        self.chore(SessionCommand::Side, question)
    }

    /// A short name for a conversation that starts with `prompt`; needs no
    /// conversation yet. Answered as [`ChoreAnswer::Title`].
    pub fn title(&self, prompt: &str) -> Result<Run, Error> {
        let mut request = PromptRequest::plain(self.cwd().to_owned(), prompt, self.model().clone());
        request.command = Some(SessionCommand::Title);
        self.start(request)
    }

    /// The prompt the user would likely type next, or nothing when there is
    /// no obvious one. Answered as [`ChoreAnswer::Suggestion`].
    pub fn suggest(&self) -> Result<Run, Error> {
        self.chore(SessionCommand::Suggest, "")
    }

    /// Where the conversation stands, for someone coming back to it.
    /// Answered as [`ChoreAnswer::Recap`].
    pub fn recap(&self) -> Result<Run, Error> {
        self.chore(SessionCommand::Recap, "")
    }

    /// The one thing the user should really know about the conversation,
    /// if there is one (most of the time there is not). Answered as
    /// [`ChoreAnswer::HeadsUp`].
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
    /// Runs a [`Session::side`] question to the end and returns its answer.
    pub fn side_answer(self) -> Result<String, Error> {
        match self.chore_answer()? {
            Some(ChoreAnswer::Side(text)) => Ok(text),
            _ => Err(Error::Run("the side question got no answer".into())),
        }
    }

    /// Runs a side call to the end and returns its answer, if it gave one.
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
