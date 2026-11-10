//! The owner's MCP servers for a folder: listing them with where they come
//! from and whether they start, approving a project's `.mcp.json` server
//! (or turning it off), and adding or removing the owner's own.

use std::path::Path;

use superfik_core::mcp_config::McpServerInfo;
use superfik_protocol::{ClientCommand, ServerEvent};

use crate::{Error, Superfik};

impl Superfik {
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
}

fn listed(event: ServerEvent) -> Option<Result<Vec<McpServerInfo>, String>> {
    match event {
        ServerEvent::McpServers { servers, .. } => Some(Ok(servers)),
        _ => None,
    }
}
