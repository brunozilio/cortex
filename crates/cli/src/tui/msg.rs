use sdk::{EngineEvent, Model};

pub enum Msg {
    Ready { models: Result<Vec<Model>, String>, settings: serde_json::Value },
    Event(domain::RunId, EngineEvent),
    Skills(Vec<(String, String)>),
    Reloaded { skills: Vec<(String, String)>, summary: String },
    Connected(String),
    Clipboard(Result<Option<sdk::Image>, String>),
    Changes(Result<super::git::Changes, String>),
    Diffs(super::git::Scope, Result<super::git::Changes, String>),
    Side { question: String, answer: Result<String, String> },
    Chore(Result<Option<sdk::ChoreAnswer>, String>),
    Files(Vec<String>),
    StatusLine(String),
    Providers(Vec<domain::provider::ProviderSummary>),
    Menu(Vec<domain::provider::ProviderSummary>),
    Conversations(Vec<super::conversations::Conversation>),
    Context(domain::usage::ContextUsage),
    Shell { id: String, command: String, result: Result<(String, bool), String> },
    Notices(Vec<domain::inbox::Notice>),
    #[allow(dead_code, reason = "the feature folders send it, and any of them may be removed")]
    Feature(Box<dyn FnOnce(&mut super::app::App) + Send>),
    Notice(String),
    Error(String),
}

impl super::app::App {
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
            Msg::Menu(providers) => self.menu(providers),
            Msg::StatusLine(line) => self.status.text = self.status.command.is_some().then_some(line),
            Msg::Context(usage) => {
                self.window = Some(usage.window);
                self.transcript.notice(super::context::view(&usage));
            }
            Msg::Shell { id, command, result } => super::shell::finished(self, &id, command, result),
            Msg::Notices(notices) => super::notices::arrived(self, notices),
            Msg::Feature(apply) => apply(self),
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Error(text) => self.transcript.error(text),
        }
    }
}
