use std::path::Path;

use crate::api::features::channels::ChannelsRequest;
use crate::api::{ClientCommand, Reply};
use domain::mcp_config::{McpEntry, McpListing, McpServerInfo};

use crate::{Client, Error};

impl Client {
    pub fn mcp_servers(&self, cwd: &Path) -> Result<Vec<McpServerInfo>, Error> {
        self.dispatcher.call(ClientCommand::ListMcpServers { path: cwd.to_path_buf() }, listed)
    }

    pub fn approve_mcp_server(&self, cwd: &Path, name: &str, approve: bool) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::ApproveMcpServer { path: cwd.to_path_buf(), name: name.to_owned(), approve };
        self.dispatcher.call(command, listed)
    }

    pub fn mcp_listing(&self, cwd: &Path) -> Result<McpListing, Error> {
        self.dispatcher.call(ClientCommand::ListMcpServers { path: cwd.to_path_buf() }, |reply| match reply {
            Reply::McpServers { servers, unreadable, .. } => Some(Ok(McpListing { servers, unreadable })),
            _ => None,
        })
    }

    pub fn mcp_server(&self, cwd: &Path, name: &str) -> Result<McpEntry, Error> {
        let command = ClientCommand::GetMcpServer { path: cwd.to_path_buf(), name: name.to_owned() };
        self.dispatcher.call(command, |reply| match reply {
            Reply::McpServer(entry) => Some(Ok(entry)),
            _ => None,
        })
    }

    pub fn add_mcp_server(
        &self,
        cwd: &Path,
        name: &str,
        server: serde_json::Value,
        replace: bool,
    ) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::AddMcpServer { path: cwd.to_path_buf(), name: name.to_owned(), server, replace };
        self.dispatcher.call(command, listed)
    }

    pub fn remove_mcp_server(&self, cwd: &Path, name: &str) -> Result<Vec<McpServerInfo>, Error> {
        self.dispatcher.call(ClientCommand::RemoveMcpServer { path: cwd.to_path_buf(), name: name.to_owned() }, listed)
    }

    pub fn sign_in_mcp_server(
        &self,
        cwd: &Path,
        name: &str,
        mut open: impl FnMut(&str),
    ) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::SignInMcpServer { path: cwd.to_path_buf(), name: name.to_owned() };
        self.dispatcher.flow(command, crate::client::dispatch::sign_in_patience(), |reply| {
            if let Reply::McpAuthorizeAt { url, .. } = &reply {
                open(url);
                return None;
            }
            listed(reply)
        })
    }

    pub fn open_channels(&self, cwd: &Path, names: &[String]) -> Result<(Vec<String>, Vec<String>), Error> {
        let request = ChannelsRequest { path: cwd.to_path_buf(), names: names.to_vec() };
        self.ask_channels(request).map(|reply| (reply.opened, reply.failed))
    }

    pub fn sign_out_mcp_server(&self, cwd: &Path, name: &str) -> Result<Vec<McpServerInfo>, Error> {
        self.dispatcher.call(ClientCommand::SignOutMcpServer { path: cwd.to_path_buf(), name: name.to_owned() }, listed)
    }
}

impl Client {
    pub fn notices(&self, for_agent: bool) -> Result<crate::api::Notices, Error> {
        self.dispatcher.call(ClientCommand::TakeNotices { for_agent }, |reply| match reply {
            Reply::Notices(notices) => Some(Ok(notices)),
            _ => None,
        })
    }

    pub fn answer_teammate(&self, ask: domain::team::TeammateAsk, decision: domain::Decision) -> Result<(), Error> {
        self.dispatcher.call(ClientCommand::AnswerTeammate { ask, decision }, |reply| match reply {
            Reply::Done => Some(Ok(())),
            Reply::Error { message } => Some(Err(message)),
            _ => None,
        })
    }

    pub fn trust_state(&self, cwd: &Path) -> Result<domain::trust::TrustReport, Error> {
        self.dispatcher.call(ClientCommand::TrustState { path: cwd.to_path_buf() }, trust_report)
    }

    pub fn set_trust(&self, cwd: &Path, trusted: bool) -> Result<domain::trust::TrustReport, Error> {
        self.dispatcher.call(ClientCommand::Trust { path: cwd.to_path_buf(), trusted }, trust_report)
    }

    pub fn trust_for_this_process(&self, cwd: &Path) -> Result<domain::trust::TrustReport, Error> {
        self.dispatcher.call(ClientCommand::TrustForThisProcess { path: cwd.to_path_buf() }, trust_report)
    }

    pub fn permissions(&self, cwd: &Path) -> Result<domain::permission_rules::PermissionsInfo, Error> {
        self.dispatcher.call(ClientCommand::ListPermissions { path: cwd.to_path_buf() }, |reply| match reply {
            Reply::Permissions { permissions, .. } => Some(Ok(permissions)),
            _ => None,
        })
    }
}

fn trust_report(reply: Reply) -> Option<Result<domain::trust::TrustReport, String>> {
    match reply {
        Reply::Trust(report) => Some(Ok(report)),
        _ => None,
    }
}

fn listed(reply: Reply) -> Option<Result<Vec<McpServerInfo>, String>> {
    match reply {
        Reply::McpServers { servers, .. } => Some(Ok(servers)),
        _ => None,
    }
}
