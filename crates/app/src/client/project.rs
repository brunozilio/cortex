use std::path::{Path, PathBuf};

use crate::api::features::code_map::CodeMapRequest;
use crate::api::features::declared_providers::DeclaredProvidersRequest;
use crate::api::features::flags::{FlagsRequest, ListedFlag};
use crate::api::features::plugins::{PluginsReply, PluginsRequest};
use crate::api::features::schedules::{SchedulesReply, SchedulesRequest};
use crate::api::features::shell::{BackgroundCommand, ShellReply, ShellRequest};
use crate::api::features::suggested_rules::{SuggestedRulesReply, SuggestedRulesRequest};
use crate::api::features::trace::{TraceReply, TraceRequest};
use domain::SessionId;
use domain::output::Saving;
use domain::permission_rules::RuleKind;
use domain::schedule::{Scheduled, When};
use domain::suggested_rules::Suggestion;

use crate::client::features::unexpected;
use crate::{Client, Error};

impl Client {
    pub fn run_typed(&self, cwd: &Path, command: &str) -> Result<(String, bool), Error> {
        match self.ask_shell(ShellRequest::Run { command: command.to_owned(), path: cwd.to_path_buf() })? {
            ShellReply::Ran { output, failed } => Ok((output, failed)),
            _ => Err(unexpected()),
        }
    }

    pub fn background_commands(&self) -> Result<Vec<BackgroundCommand>, Error> {
        self.listed_commands(ShellRequest::List)
    }

    pub fn kill_background_command(&self, id: &str) -> Result<Vec<BackgroundCommand>, Error> {
        self.listed_commands(ShellRequest::Kill { id: id.to_owned() })
    }

    pub fn background_output(&self, id: &str) -> Result<String, Error> {
        match self.ask_shell(ShellRequest::Output { id: id.to_owned() })? {
            ShellReply::Output(output) => Ok(output),
            _ => Err(unexpected()),
        }
    }

    fn listed_commands(&self, request: ShellRequest) -> Result<Vec<BackgroundCommand>, Error> {
        match self.ask_shell(request)? {
            ShellReply::Listed(commands) => Ok(commands),
            _ => Err(unexpected()),
        }
    }

    pub fn plugins(&self, request: PluginsRequest) -> Result<PluginsReply, Error> {
        self.ask_plugins(request)
    }

    pub fn flags(&self, cwd: &Path) -> Result<Vec<ListedFlag>, Error> {
        self.ask_flags(FlagsRequest::List { cwd: cwd.to_path_buf() }).map(|reply| reply.flags)
    }

    pub fn set_flag(&self, cwd: &Path, name: &str, on: bool) -> Result<Vec<ListedFlag>, Error> {
        let request = FlagsRequest::Set { cwd: cwd.to_path_buf(), name: name.to_owned(), on };
        self.ask_flags(request).map(|reply| reply.flags)
    }

    pub fn scheduled(&self) -> Result<Vec<Scheduled>, Error> {
        self.ask_schedules(SchedulesRequest::List).map(|reply| reply.scheduled)
    }

    pub fn take_due_schedules(&self) -> Result<Vec<Scheduled>, Error> {
        self.ask_schedules(SchedulesRequest::TakeDue).map(|reply| reply.due)
    }

    pub fn delete_schedule(&self, id: &str) -> Result<Vec<Scheduled>, Error> {
        self.ask_schedules(SchedulesRequest::Delete { id: id.to_owned() }).map(|reply| reply.scheduled)
    }

    pub fn schedule(&self, prompt: &str, when: When) -> Result<SchedulesReply, Error> {
        self.ask_schedules(SchedulesRequest::Create { prompt: prompt.to_owned(), when })
    }

    pub fn suggested_rules(&self, cwd: &Path) -> Result<Vec<Suggestion>, Error> {
        match self.ask_suggested_rules(SuggestedRulesRequest::List { cwd: cwd.to_path_buf() })? {
            SuggestedRulesReply::Suggested(suggestions) => Ok(suggestions),
            _ => Err(unexpected()),
        }
    }

    pub fn keep_rule(&self, cwd: &Path, kind: RuleKind, rule: &str) -> Result<(), Error> {
        let request = SuggestedRulesRequest::KeepRule { cwd: cwd.to_path_buf(), kind, rule: rule.to_owned() };
        match self.ask_suggested_rules(request)? {
            SuggestedRulesReply::RuleKept => Ok(()),
            _ => Err(unexpected()),
        }
    }

    pub fn keep_instruction(&self, cwd: &Path, line: &str) -> Result<PathBuf, Error> {
        let request = SuggestedRulesRequest::KeepInstruction { cwd: cwd.to_path_buf(), line: line.to_owned() };
        match self.ask_suggested_rules(request)? {
            SuggestedRulesReply::InstructionKept { file } => Ok(file),
            _ => Err(unexpected()),
        }
    }

    pub fn trace_of_every_session(&self) -> Result<serde_json::Value, Error> {
        self.trace_report(TraceRequest::EverySession)
    }

    pub fn trace_of_session(&self, session: &SessionId, cwd: &Path) -> Result<serde_json::Value, Error> {
        self.trace_report(TraceRequest::Session { session: session.clone(), cwd: cwd.to_path_buf() })
    }

    pub fn followed_trace(&self, session: &SessionId, cwd: &Path) -> Result<serde_json::Value, Error> {
        match self.ask_trace(TraceRequest::Follow { session: session.clone(), cwd: cwd.to_path_buf() })? {
            TraceReply::Followed(update) => Ok(update),
            TraceReply::Report(_) => Err(unexpected()),
        }
    }

    fn trace_report(&self, request: TraceRequest) -> Result<serde_json::Value, Error> {
        match self.ask_trace(request)? {
            TraceReply::Report(report) => Ok(report),
            TraceReply::Followed(_) => Err(unexpected()),
        }
    }

    pub fn code_map(&self, cwd: &Path, prompt: &str) -> Result<Option<String>, Error> {
        let request = CodeMapRequest { path: cwd.to_path_buf(), prompt: prompt.to_owned() };
        self.ask_code_map(request).map(|reply| reply.map)
    }

    pub fn declare_providers(&self, cwd: &Path) -> Result<Vec<String>, Error> {
        self.ask_declared_providers(DeclaredProvidersRequest { path: cwd.to_path_buf() }).map(|reply| reply.lines)
    }

    pub fn output_savings(&self) -> Result<Vec<Saving>, Error> {
        self.ask_output_savings().map(|reply| reply.savings)
    }
}
