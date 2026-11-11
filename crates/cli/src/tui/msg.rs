//! What slow work on other threads sends back to the chat.

use sdk::{EngineEvent, Model};

pub enum Msg {
    Ready {
        models: Result<Vec<Model>, String>,
        settings: serde_json::Value,
    },
    Event(domain::RunId, EngineEvent),
    Skills(Vec<(String, String)>),
    /// What `/reload` found: the skills to complete and what to say.
    Reloaded {
        skills: Vec<(String, String)>,
        summary: String,
    },
    Connected(String),
    /// What Ctrl+V found on the clipboard.
    Clipboard(Result<Option<sdk::Image>, String>),
    Changes(Result<super::git::Changes, String>),
    /// What `/diffs` asked git for.
    Diffs(super::git::Scope, Result<super::git::Changes, String>),
    Side {
        question: String,
        answer: Result<String, String>,
    },
    /// A side call the chat made by itself (`chores`).
    Chore(Result<Option<sdk::ChoreAnswer>, String>),
    /// The project's files, for `@` completion.
    Files(Vec<String>),
    /// What the `/statusline` command printed.
    StatusLine(String),
    /// The connected providers, for `/logout`.
    Providers(Vec<domain::provider::ModelProvider>),
    /// The folder's earlier conversations, for `/resume`.
    Conversations(Vec<super::conversations::Conversation>),
    /// What `/context` measured.
    Context(domain::usage::ContextUsage),
    /// A `!command` ended.
    Shell {
        id: String,
        command: String,
        result: Result<(String, bool), String>,
    },
    /// Scheduled prompts now due, and what reached the inbox, to send.
    Due(Vec<domain::schedule::Scheduled>, Vec<domain::inbox::Notice>),
    Notice(String),
    Error(String),
}

impl super::app::App {
    /// What came back from other threads.
    pub fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Ready { models, settings } => self.ready(models, &settings),
            Msg::Event(run, event) => self.event(run, event),
            Msg::Skills(skills) => self.skills = skills,
            Msg::Reloaded { skills, summary } => {
                self.skills = skills;
                self.transcript.notice(summary);
            }
            Msg::Connected(provider) => {
                self.transcript.notice(format!("Connected {provider}."));
                self.load(false);
            }
            Msg::Clipboard(found) => self.clipboard(found),
            Msg::Changes(changes) => self.changes(changes),
            Msg::Diffs(scope, changes) => {
                if let Some(view) = &mut self.diffs {
                    view.loaded(scope, changes);
                }
            }
            Msg::Side { question, answer } => super::side::answered(self, &question, answer),
            Msg::Chore(answer) => self.chore_answered(answer),
            Msg::Files(files) => self.files = Some(files),
            Msg::Conversations(found) => super::resume::listed(self, found),
            Msg::Providers(providers) => super::info::connected(self, providers),
            Msg::StatusLine(line) => self.status.text = self.status.command.is_some().then_some(line),
            Msg::Context(usage) => {
                self.window = Some(usage.window);
                self.transcript.notice(super::context::view(&usage));
            }
            Msg::Shell { id, command, result } => super::shell::finished(self, &id, command, result),
            Msg::Due(due, notices) => super::schedules::due(self, due, notices),
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Error(text) => self.transcript.error(text),
        }
    }
}
