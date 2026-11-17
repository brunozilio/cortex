use app::{EngineEvent, Model};

use super::fleet;
use super::turn::Ticket;

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
    PortBusy(super::sign_in::PortBusy),
    Prepared { ticket: Ticket, outgoing: super::turn_flow::Outgoing },
    SteerResolved { run: domain::RunId, text: String },
    GitOffer { ticket: Ticket, offered: bool, text: String },
    GitInitialized { ticket: Ticket, committed: Result<bool, String>, text: String },
    Nothing,
    Accounts { providers: Vec<domain::provider::ProviderSummary>, removing: bool },
    Conversations(Vec<super::conversations::Conversation>),
    Earlier(Vec<super::conversations::Conversation>),
    Context(domain::usage::ContextUsage),
    Shell { id: String, command: String, result: Result<(String, bool), String> },
    Notices(app::api::Notices),
    Feature(super::features::Reply),
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
            Msg::Clipboard(found) => super::attach::clipboard_read(self, found),
            Msg::Changes(changes) => super::git::shown(self, changes),
            Msg::Diffs(scope, changes) => {
                if let Some(view) = &mut self.overlays.diffs {
                    view.loaded(scope, changes);
                }
            }
            Msg::Side { question, answer } => super::side::answered(self, &question, answer),
            Msg::Chore(session, answer) => super::fleet::chore(self, session, answer),
            Msg::Files(files) => self.catalog.files = Some(files),
            Msg::Earlier(found) => self.fleet.earlier = found,
            Msg::Conversations(found) => super::resume::listed(self, found),
            Msg::Providers(providers) => super::accounts::connected(self, providers),
            Msg::Menu(providers) => super::sign_in::menu(self, providers),
            Msg::PortBusy(busy) => super::sign_in::port_busy(self, busy),
            Msg::Prepared { ticket, outgoing } => {
                fleet::for_turn(self, |turn| turn.waits_for(ticket), |app| app.prepared(ticket, outgoing));
            }
            Msg::SteerResolved { run, text } => {
                fleet::for_turn(self, |turn| turn.runs(run), |app| app.steer_resolved(run, text));
            }
            Msg::GitOffer { ticket, offered, text } => {
                fleet::for_turn(
                    self,
                    |turn| turn.waits_for(ticket),
                    |app| super::git_offer::checked(app, ticket, offered, text),
                );
            }
            Msg::GitInitialized { ticket, committed, text } => {
                fleet::for_turn(
                    self,
                    |turn| turn.waits_for(ticket),
                    |app| super::git_offer::initialized(app, ticket, committed, text),
                );
            }
            Msg::Accounts { providers, removing } => super::accounts::listed(self, providers, removing),
            Msg::StatusLine(line) => super::statusline::arrived(self, line),
            Msg::Context(usage) => {
                self.session.window = Some(usage.window);
                self.transcript.notice(super::context::view(&usage));
            }
            Msg::Shell { id, command, result } => super::typed_shell::finished(self, &id, command, result),
            Msg::Notices(notices) => super::notices::arrived(self, notices),
            Msg::Feature(reply) => reply.apply(self),
            Msg::Nothing => {}
            Msg::Notice(text) => self.transcript.notice(text),
            Msg::Passing(text) => self.transcript.passing(text),
            Msg::Error(text) => self.transcript.error(text),
        }
    }
}
