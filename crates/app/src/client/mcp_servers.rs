use std::path::Path;

use crate::api::features::channels::ChannelsRequest;
use crate::api::{ClientCommand, Reply};
use domain::mcp_config::McpServerInfo;

use crate::{Error, Sdk};

impl Sdk {
    pub fn mcp_servers(&self, cwd: &Path) -> Result<Vec<McpServerInfo>, Error> {
        self.client.call(ClientCommand::ListMcpServers { path: cwd.to_path_buf() }, listed)
    }

    pub fn approve_mcp_server(&self, cwd: &Path, name: &str, approve: bool) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::ApproveMcpServer { path: cwd.to_path_buf(), name: name.to_owned(), approve };
        self.client.call(command, listed)
    }

    pub fn add_mcp_server(
        &self,
        cwd: &Path,
        name: &str,
        server: serde_json::Value,
    ) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::AddMcpServer { path: cwd.to_path_buf(), name: name.to_owned(), server };
        self.client.call(command, listed)
    }

    pub fn remove_mcp_server(&self, cwd: &Path, name: &str) -> Result<Vec<McpServerInfo>, Error> {
        self.client.call(ClientCommand::RemoveMcpServer { path: cwd.to_path_buf(), name: name.to_owned() }, listed)
    }

    pub fn sign_in_mcp_server(
        &self,
        cwd: &Path,
        name: &str,
        mut open: impl FnMut(&str),
    ) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::SignInMcpServer { path: cwd.to_path_buf(), name: name.to_owned() };
        self.client.flow(command, crate::client::dispatch::sign_in_patience(), |reply| {
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
        self.client.call(ClientCommand::SignOutMcpServer { path: cwd.to_path_buf(), name: name.to_owned() }, listed)
    }
}

impl Sdk {
    pub fn notices(&self) -> Result<Vec<domain::inbox::Notice>, Error> {
        self.client.call(ClientCommand::TakeNotices, |reply| match reply {
            Reply::Notices(notices) => Some(Ok(notices)),
            _ => None,
        })
    }

    pub fn permissions(&self, cwd: &Path) -> Result<domain::permission_rules::PermissionsInfo, Error> {
        self.client.call(ClientCommand::ListPermissions { path: cwd.to_path_buf() }, |reply| match reply {
            Reply::Permissions { permissions, .. } => Some(Ok(permissions)),
            _ => None,
        })
    }
}

fn listed(reply: Reply) -> Option<Result<Vec<McpServerInfo>, String>> {
    match reply {
        Reply::McpServers { servers, .. } => Some(Ok(servers)),
        _ => None,
    }
}
