use crate::api::features::channels::{ChannelsReply, ChannelsRequest};
use crate::api::features::code_map::{CodeMapReply, CodeMapRequest};
use crate::api::features::declared_providers::{DeclaredProvidersReply, DeclaredProvidersRequest};
use crate::api::features::flags::{FlagsReply, FlagsRequest};
use crate::api::features::memory::{MemoryReply, MemoryRequest};
use crate::api::features::output_savings::OutputSavingsReply;
use crate::api::features::plugins::{PluginsReply, PluginsRequest};
use crate::api::features::schedules::{SchedulesReply, SchedulesRequest};
use crate::api::features::sessions::{SessionsReply, SessionsRequest};
use crate::api::features::shell::{ShellReply, ShellRequest};
use crate::api::features::suggested_rules::{SuggestedRulesReply, SuggestedRulesRequest};
use crate::api::features::trace::{TraceReply, TraceRequest};
use crate::api::features::workspace::{WorkspaceReply, WorkspaceRequest};
use crate::api::{ClientCommand, FeatureReply, FeatureRequest, Reply};

use crate::{Client, Error};

pub(crate) fn unexpected() -> Error {
    Error::Backend("the backend answered something else".into())
}

impl Client {
    fn feature_reply(&self, request: FeatureRequest) -> Result<FeatureReply, Error> {
        let answered = self.dispatcher.call(ClientCommand::Feature(request), |reply| match reply {
            Reply::Feature(reply) => Some(Ok(Ok(reply))),
            Reply::FeatureFailed(error) => Some(Ok(Err(error))),
            _ => None,
        })?;
        answered.map_err(Error::Feature)
    }

    pub(crate) fn ask_output_savings(&self) -> Result<OutputSavingsReply, Error> {
        match self.feature_reply(FeatureRequest::OutputSavings)? {
            FeatureReply::OutputSavings(reply) => Ok(reply),
            _ => Err(unexpected()),
        }
    }
}

macro_rules! asks {
    ($($method:ident: $feature:ident($request:ty) -> $reply:ty;)*) => {
        impl Client {
            $(pub(crate) fn $method(&self, request: $request) -> Result<$reply, Error> {
                match self.feature_reply(FeatureRequest::$feature(request))? {
                    FeatureReply::$feature(reply) => Ok(reply),
                    _ => Err(unexpected()),
                }
            })*
        }
    };
}

asks! {
    ask_channels: Channels(ChannelsRequest) -> ChannelsReply;
    ask_code_map: CodeMap(CodeMapRequest) -> CodeMapReply;
    ask_declared_providers: DeclaredProviders(DeclaredProvidersRequest) -> DeclaredProvidersReply;
    ask_flags: Flags(FlagsRequest) -> FlagsReply;
    ask_memory: Memory(MemoryRequest) -> MemoryReply;
    ask_plugins: Plugins(PluginsRequest) -> PluginsReply;
    ask_schedules: Schedules(SchedulesRequest) -> SchedulesReply;
    ask_sessions: Sessions(SessionsRequest) -> SessionsReply;
    ask_shell: Shell(ShellRequest) -> ShellReply;
    ask_suggested_rules: SuggestedRules(SuggestedRulesRequest) -> SuggestedRulesReply;
    ask_trace: Trace(TraceRequest) -> TraceReply;
    ask_workspace: Workspace(WorkspaceRequest) -> WorkspaceReply;
}
