use app::{EngineEvent, Model};

pub enum Msg {
    Ready { models: Result<Vec<Model>, String>, settings: serde_json::Value },
    Event(domain::RunId, EngineEvent),
    Skills(Vec<(String, String)>),
    Reloaded { skills: Vec<(String, String)>, summary: String },
    Connected(String),
    Clipboard(Result<Option<app::Image>, String>),
    Changes(Result<super::git::Changes, String>),
    Diffs(super::git::Scope, Result<super::git::Changes, String>),
    Side { question: String, answer: Result<String, String> },
    Chore(Option<app::SessionId>, Result<Option<app::ChoreAnswer>, String>),
    Files(Vec<String>),
    StatusLine(String),
    Providers(Vec<domain::provider::ProviderSummary>),
    Menu(Vec<domain::provider::ProviderSummary>),
    Accounts { providers: Vec<domain::provider::ProviderSummary>, removing: bool },
    Conversations(Vec<super::conversations::Conversation>),
    Earlier(Vec<super::conversations::Conversation>),
    Context(domain::usage::ContextUsage),
    Shell { id: String, command: String, result: Result<(String, bool), String> },
    Notices(Vec<domain::inbox::Notice>),
    Feature(Box<dyn FnOnce(&mut super::app::App) + Send>),
    Notice(String),
    Passing(String),
    Error(String),
}

impl super::app::App {
    pub fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Ready { models, settings } => self.ready(models, &settings),
            Msg::Event(run, event) => super::fleet::event(self, run, event),
            Msg::Skills(skills) => self.catalog.skills = skills,
            Msg::Reloaded { skills, summary } => {
                self.catalog.skills = skills;
                self.transcript.notice(summary);
            }
            Msg::Connected(provider) => {
                self.transcript.replace_passing(format!("Connected {provider}."));
                self.load(false);
            }
            Msg::Clipboard(found) => self.clipboard(found),
            Msg::Changes(changes) => self.changes(changes),
            Msg::Diffs(scope, changes) => {
                if let Some(view) = &mut self.overlays.diffs {
                    view.loaded(scope, changes);
                }
            }
            Msg::Side { question, answer } => super::side::answered(self, &question, answer),
            Msg::Chore(session, answer) => super::fleet::chore(self, session, answer),
            Msg::Files(files) => self.catalog.files = Some(files),
            Msg::Conversations(found) => super::resume::listed(self, found),
            Msg::Earlier(found) => self.fleet.earlier = found,
            Msg::Providers(providers) => super::info::connected(self, providers),
            Msg::Menu(providers) => self.menu(providers),
            Msg::Accounts { providers, removing } => self.accounts_listed(providers, removing),
            Msg::StatusLine(line) => super::statusline::arrived(self, line),
            Msg::Context(usage) => {
                self.window = Some(usage.window);
                self.transcript.notice(super::context::view(&usage));
            }
            Msg::Shell { id, command, result } => super::shell::finished(self, &id, command, result),
            Msg::Notices(notices) => super::notices::arrived(self, notices),
            Msg::Feature(apply) => apply(self),
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Passing(text) => self.transcript.passing(text),
            Msg::Error(text) => self.transcript.error(text),
        }
    }
}
