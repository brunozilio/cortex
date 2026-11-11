//! The owner's MCP servers for a folder: listing them with where they come
//! from and whether they start, approving a project's `.mcp.json` server
//! (or turning it off), and adding or removing the owner's own.

use std::path::Path;

use domain::mcp_config::McpServerInfo;
use protocol::{ClientCommand, ServerEvent};

use crate::{Error, Sdk};

impl Sdk {
    /// The MCP servers a session in `cwd` can use.
    pub fn mcp_servers(&self, cwd: &Path) -> Result<Vec<McpServerInfo>, Error> {
        self.client.call(ClientCommand::ListMcpServers { path: cwd.to_path_buf() }, listed)
    }

    /// Approves a project's server (`true`) or turns it off (`false`).
    pub fn approve_mcp_server(&self, cwd: &Path, name: &str, approve: bool) -> Result<Vec<McpServerInfo>, Error> {
        let command = ClientCommand::ApproveMcpServer { path: cwd.to_path_buf(), name: name.to_owned(), approve };
        self.client.call(command, listed)
    }

    /// Adds or replaces one of the owner's own servers, given as in
    /// `mcpServers`: `{"command": …, "args": […], "env": {…}}` or
    /// `{"type": "http", "url": …, "headers": {…}}`.
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

    /// Signs in to the remote MCP server `name` in the browser: `open` gets
    /// the address to visit; returns the listing once signed in. Runs then
    /// send the server the owner's token, refreshed when stale.
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

    /// Keeps the MCP servers `names` up as this client's channels: their
    /// messages reach its sessions as events. The channels open, and why
    /// the others are not.
    pub fn open_channels(&self, cwd: &Path, names: &[String]) -> Result<(Vec<String>, Vec<String>), Error> {
        let command = ClientCommand::OpenChannels { path: cwd.to_path_buf(), names: names.to_vec() };
        self.client.call(command, |event| match event {
            ServerEvent::Channels { opened, failed } => Some(Ok((opened, failed))),
            _ => None,
        })
    }

    pub fn sign_out_mcp_server(&self, cwd: &Path, name: &str) -> Result<Vec<McpServerInfo>, Error> {
        self.client.call(ClientCommand::SignOutMcpServer { path: cwd.to_path_buf(), name: name.to_owned() }, listed)
    }
}

impl Sdk {
    /// The commands this client's runs left in the background (id,
    /// command, still running), after stopping `kill` when given.
    pub fn shells(&self, kill: Option<&str>) -> Result<Vec<(String, String, bool)>, Error> {
        let command = ClientCommand::ListShells { kill: kill.map(str::to_owned) };
        self.client.call(command, |event| match event {
            ServerEvent::Shells(shells) => Some(Ok(shells)),
            _ => None,
        })
    }

    /// The prompts this client's runs scheduled: those due now when
    /// `take_due` (handed over once; send them), and all still waiting,
    /// after deleting `delete` when given.
    pub fn schedules(&self, delete: Option<&str>, take_due: bool) -> Result<Schedules, Error> {
        let command = ClientCommand::Schedules { delete: delete.map(str::to_owned), take_due };
        self.client.call(command, |event| match event {
            ServerEvent::Schedules { due, scheduled, notices } => Some(Ok(Schedules { due, scheduled, notices })),
            _ => None,
        })
    }

    /// The permission rules (deny, ask, then allow, as written) and added
    /// folders a session in `cwd` follows.
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

/// What [`the app::schedules`] answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedules {
    pub due: Vec<domain::schedule::Scheduled>,
    pub scheduled: Vec<domain::schedule::Scheduled>,
    /// What reached the inbox, when `take_due`: send it as a turn.
    pub notices: Vec<domain::inbox::Notice>,
}
