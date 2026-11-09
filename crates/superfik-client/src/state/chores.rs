//! Side calls about a conversation, apart from its runs: the model names
//! it after the first prompt, guesses the next prompt after each reply,
//! recaps it for a user coming back and answers `/btw` questions. Each is a
//! short run of its own (`SessionCommand::is_chore`), several at once,
//! that never touches the transcript nor waits for a prompt or a
//! measurement.

use superfik_core::{ChoreAnswer, EngineEvent, PromptRequest, RunId, SessionCommand, command};
use superfik_protocol::ClientCommand;

use super::upkeep::Upkeep;
use super::{Effect, Notice, Session};

const HEADS_UP_STEPS: usize = 8;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chores {
    pub running: Vec<Upkeep>,
    pub suggestion: Option<String>,
    pub guessed_title: Option<String>,
    pub recapped: bool,
    pub title_from: Option<String>,
}

impl Chores {
    pub(super) fn owns(&self, run: RunId) -> bool {
        self.running.iter().any(|chore| chore.run == run)
    }
}

impl Session {
    pub(super) fn start_chore(&mut self, index: usize, command: SessionCommand, prompt: String) -> Vec<Effect> {
        let Some(conversation) = self.conversations.get(index) else { return Vec::new() };
        let resume = conversation.engine_session.clone().filter(|_| command != SessionCommand::Title);
        if command != SessionCommand::Title && resume.is_none() {
            return Vec::new();
        }
        let Some(request) = self.session_request(index, command, prompt, resume) else { return Vec::new() };
        let run = RunId::new();
        self.conversations[index].chores.running.push(Upkeep { run, command });
        vec![Effect::Command(Box::new(ClientCommand::SubmitPrompt { run, request }))]
    }

    pub(super) fn session_request(
        &self,
        index: usize,
        command: SessionCommand,
        prompt: String,
        resume: Option<superfik_core::SessionId>,
    ) -> Option<PromptRequest> {
        let (root, conversation) = (self.root.clone()?, self.conversations.get(index)?);
        if self.models.is_empty() {
            return None;
        }
        let mut request = PromptRequest::plain(root, prompt, self.model_of(index).clone());
        request.workspace = self.workspace_id;
        request.effort = conversation.effort;
        request.resume = resume;
        request.command = Some(command);
        Some(request)
    }

    pub(super) fn chore_event(&mut self, index: usize, run: RunId, event: &EngineEvent) -> Vec<Effect> {
        let c = &mut self.conversations[index];
        let command = c.chores.running.iter().find(|chore| chore.run == run).map(|chore| chore.command);
        match event {
            EngineEvent::Chore(ChoreAnswer::Title(title)) if command == Some(SessionCommand::Rename) => {
                c.title = Some(title.clone());
            }
            EngineEvent::Chore(ChoreAnswer::Title(title)) => {
                if c.title.is_some() && c.title == c.chores.guessed_title {
                    c.title = Some(title.clone());
                }
                c.chores.guessed_title = None;
            }
            EngineEvent::Chore(ChoreAnswer::Suggestion(text)) if c.composer.text().is_empty() && !c.is_running() => {
                c.chores.suggestion = Some(text.clone());
            }
            EngineEvent::Chore(ChoreAnswer::Recap(text)) => self.notice = Some(Notice::Recap(text.clone())),
            EngineEvent::Chore(ChoreAnswer::Side(answer)) => self.notice = Some(Notice::SideAnswer(answer.clone())),
            EngineEvent::Chore(ChoreAnswer::HeadsUp(found)) => {
                self.notice.get_or_insert_with(|| Notice::HeadsUp(found.clone()));
            }
            EngineEvent::Failed(message) if command == Some(SessionCommand::Side) => {
                self.notice = Some(Notice::Raw(message.clone()));
            }
            _ => {}
        }
        if event.is_terminal() {
            self.conversations[index].chores.running.retain(|chore| chore.run != run);
        }
        Vec::new()
    }

    pub(super) fn ask_title(&mut self, index: usize) -> Vec<Effect> {
        match self.conversations[index].chores.title_from.take() {
            Some(prompt) => self.start_chore(index, SessionCommand::Title, prompt),
            None => Vec::new(),
        }
    }

    pub(super) fn suggest_next(&mut self, index: usize) -> Vec<Effect> {
        let c = &self.conversations[index];
        if !self.preferences.suggestions || self.voice.is_on() || !c.composer.text().is_empty() {
            return Vec::new();
        }
        self.start_chore(index, SessionCommand::Suggest, String::new())
    }

    pub(super) fn look_for_heads_up(&mut self, index: usize) -> Vec<Effect> {
        let c = &self.conversations[index];
        let steps = c.turns.last().map_or(0, |turn| turn.response.tools.len());
        if !self.preferences.heads_up || self.voice.is_on() || steps < HEADS_UP_STEPS {
            return Vec::new();
        }
        self.start_chore(index, SessionCommand::HeadsUp, String::new())
    }

    pub fn suggestion(&self) -> Option<&str> {
        let c = self.conversation();
        let off = !self.preferences.suggestions;
        if off || !c.composer.text().is_empty() || c.is_running() || self.renaming.is_some() {
            return None;
        }
        c.chores.suggestion.as_deref()
    }

    pub(super) fn accept_suggestion(&mut self) -> Vec<Effect> {
        let Some(text) = self.suggestion().map(str::to_owned) else { return Vec::new() };
        let c = self.conversation_mut();
        c.composer.insert(&text);
        c.chores.suggestion = None;
        Vec::new()
    }

    pub(super) fn recap_on_return(&mut self, index: usize) -> Vec<Effect> {
        let c = &self.conversations[index];
        if c.chores.recapped || c.turns.is_empty() || c.is_running() || c.shared_by.is_some() {
            return Vec::new();
        }
        self.conversations[index].chores.recapped = true;
        self.start_chore(index, SessionCommand::Recap, String::new())
    }

    pub(super) fn run_rename(&mut self) -> Option<Vec<Effect>> {
        let title = command::argument(self.conversation().composer.text(), "rename")?.to_owned();
        let c = self.conversation_mut();
        c.composer.take();
        c.title = Some(title);
        c.chores.guessed_title = None;
        Some(Vec::new())
    }

    pub(super) fn rename_from_conversation(&mut self) -> Vec<Effect> {
        let index = self.active_index();
        if self.conversations[index].engine_session.is_none() {
            self.notice = Some(Notice::NothingToAsk);
            return Vec::new();
        }
        self.start_chore(index, SessionCommand::Rename, String::new())
    }

    pub(super) fn run_side_question(&mut self) -> Option<Vec<Effect>> {
        let question = command::argument(self.conversation().composer.text(), "btw")?.to_owned();
        self.conversation_mut().composer.take();
        let index = self.active_index();
        if self.conversations[index].engine_session.is_none() {
            self.notice = Some(Notice::NothingToAsk);
            return Some(Vec::new());
        }
        Some(self.start_chore(index, SessionCommand::Side, question))
    }
}
