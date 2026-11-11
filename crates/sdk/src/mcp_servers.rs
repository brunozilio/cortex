use std::path::Path;

use domain::mcp_config::McpServerInfo;
use protocol::{ClientCommand, ServerEvent};

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
        self.client.call(command, |event| {
            if let ServerEvent::McpAuthorizeAt { url, .. } = &event {
                open(url);
                return None;
            }
            listed(event)
        })
    }

    pub fn open_channels(&self, cwd: &Path, names: &[String]) -> Result<(Vec<String>, Vec<String>), Error> {
        let answer = self.feature("channels", serde_json::json!({ "path": cwd, "names": names }))?;
        let list = |key: &str| serde_json::from_value(answer[key].clone()).unwrap_or_default();
        Ok((list("opened"), list("failed")))
    }

    pub fn sign_out_mcp_server(&self, cwd: &Path, name: &str) -> Result<Vec<McpServerInfo>, Error> {
        self.client.call(ClientCommand::SignOutMcpServer { path: cwd.to_path_buf(), name: name.to_owned() }, listed)
    }
}

impl Sdk {
    pub fn notices(&self) -> Result<Vec<domain::inbox::Notice>, Error> {
        self.client.call(ClientCommand::TakeNotices, |event| match event {
            ServerEvent::Notices(notices) => Some(Ok(notices)),
            _ => None,
        })
    }

    pub fn feature(&self, feature: &str, input: serde_json::Value) -> Result<serde_json::Value, Error> {
        let command = ClientCommand::Feature { feature: feature.to_owned(), input };
        let asked = feature.to_owned();
        self.client.call(command, move |event| match event {
            ServerEvent::Feature { feature, output } if feature == asked => Some(output),
            _ => None,
        })
    }

    pub fn permissions(&self, cwd: &Path) -> Result<domain::permission_rules::PermissionsInfo, Error> {
        self.client.call(ClientCommand::ListPermissions { path: cwd.to_path_buf() }, |event| match event {
            ServerEvent::Permissions { permissions, .. } => Some(Ok(permissions)),
            _ => None,
        })
    }
}

fn listed(event: ServerEvent) -> Option<Result<Vec<McpServerInfo>, String>> {
    match event {
        ServerEvent::McpServers { servers, .. } => Some(Ok(servers)),
        _ => None,
    }
}
