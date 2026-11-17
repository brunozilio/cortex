use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{McpOrigin, McpServerInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpUnreadable {
    pub path: PathBuf,
    pub origin: McpOrigin,
    pub error: String,
}

impl McpUnreadable {
    pub fn line(&self) -> String {
        format!("{}: {}", self.path.display(), self.error)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpListing {
    pub servers: Vec<McpServerInfo>,
    #[serde(default)]
    pub unreadable: Vec<McpUnreadable>,
}

impl McpListing {
    pub fn named(&self, name: &str) -> Result<&McpServerInfo, String> {
        self.servers.iter().find(|server| server.name == name).ok_or_else(|| unknown(name, &self.servers))
    }

    pub fn owner_s_file_is_broken(&self) -> bool {
        self.unreadable.iter().any(|file| file.origin == McpOrigin::User)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &McpUnreadable> {
        self.unreadable.iter().filter(|file| file.origin != McpOrigin::User)
    }

    pub fn errors(&self) -> impl Iterator<Item = &McpUnreadable> {
        self.unreadable.iter().filter(|file| file.origin == McpOrigin::User)
    }
}

pub fn unknown(name: &str, servers: &[McpServerInfo]) -> String {
    let known: Vec<&str> = servers.iter().map(|server| server.name.as_str()).collect();
    let known = if known.is_empty() { "none".to_owned() } else { known.join(", ") };
    format!("No MCP server `{name}` here; known: {known}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpEntry {
    pub info: McpServerInfo,
    pub file: Option<PathBuf>,
    pub entry: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_config::McpState;

    fn server(name: &str) -> McpServerInfo {
        McpServerInfo { name: name.into(), origin: McpOrigin::Project, target: "x".into(), state: McpState::Pending }
    }

    #[test]
    fn a_name_that_is_not_listed_is_refused_naming_the_known_ones() {
        let listing = McpListing { servers: vec![server("jira"), server("sonar")], unreadable: Vec::new() };
        assert_eq!(listing.named("jira").map(|found| found.name.as_str()), Ok("jira"));
        assert_eq!(listing.named("ghost"), Err("No MCP server `ghost` here; known: jira, sonar".into()));
        assert_eq!(McpListing::default().named("ghost"), Err("No MCP server `ghost` here; known: none".into()));
    }

    #[test]
    fn only_the_owner_s_own_broken_file_is_an_error() {
        let broken = |origin: McpOrigin| McpUnreadable { path: "/m.json".into(), origin, error: "eof".into() };
        let project = McpListing { servers: Vec::new(), unreadable: vec![broken(McpOrigin::Project)] };
        assert!(!project.owner_s_file_is_broken());
        assert_eq!(project.warnings().map(McpUnreadable::line).collect::<Vec<_>>(), ["/m.json: eof"]);
        let user = McpListing { servers: Vec::new(), unreadable: vec![broken(McpOrigin::User)] };
        assert!(user.owner_s_file_is_broken() && user.warnings().next().is_none());
    }
}
