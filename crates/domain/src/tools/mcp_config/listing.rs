use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{McpOrigin, McpServerInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpUnreadable {
    pub path: PathBuf,
    pub origin: McpOrigin,
    pub error: String,
    #[serde(default)]
    pub rest_kept: bool,
}

impl McpUnreadable {
    pub fn broken(path: PathBuf, origin: McpOrigin, error: String) -> Self {
        Self { path, origin, error, rest_kept: false }
    }

    pub fn left_out(path: PathBuf, origin: McpOrigin, problem: &str) -> Self {
        Self { path, origin, error: problem.to_owned(), rest_kept: true }
    }

    pub fn line(&self) -> String {
        format!("{}: {}", self.path.display(), self.error)
    }

    fn breaks_the_owner_s_file(&self) -> bool {
        self.origin == McpOrigin::User && !self.rest_kept
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
        self.unreadable.iter().any(McpUnreadable::breaks_the_owner_s_file)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &McpUnreadable> {
        self.unreadable.iter().filter(|file| !file.breaks_the_owner_s_file())
    }

    pub fn errors(&self) -> impl Iterator<Item = &McpUnreadable> {
        self.unreadable.iter().filter(|file| file.breaks_the_owner_s_file())
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
        let broken = |origin: McpOrigin| McpUnreadable::broken("/m.json".into(), origin, "eof".into());
        let project = McpListing { servers: Vec::new(), unreadable: vec![broken(McpOrigin::Project)] };
        assert!(!project.owner_s_file_is_broken());
        assert_eq!(project.warnings().map(McpUnreadable::line).collect::<Vec<_>>(), ["/m.json: eof"]);
        let user = McpListing { servers: Vec::new(), unreadable: vec![broken(McpOrigin::User)] };
        assert!(user.owner_s_file_is_broken() && user.warnings().next().is_none());
    }

    #[test]
    fn a_value_left_out_of_the_owner_s_file_is_a_warning_not_a_broken_file() {
        let left_out = McpUnreadable::left_out("/m.json".into(), McpOrigin::User, "mcpServers.x.args: expected array");
        let listing = McpListing { servers: Vec::new(), unreadable: vec![left_out] };
        assert!(!listing.owner_s_file_is_broken() && listing.errors().next().is_none());
        assert_eq!(
            listing.warnings().map(McpUnreadable::line).collect::<Vec<_>>(),
            ["/m.json: mcpServers.x.args: expected array"]
        );
    }

    #[test]
    fn an_entry_written_before_values_could_be_left_out_reads_as_a_broken_file() {
        let entry: McpUnreadable =
            serde_json::from_value(serde_json::json!({"path": "/m.json", "origin": {"kind": "user"}, "error": "eof"}))
                .expect("entry");
        assert!(!entry.rest_kept);
    }
}
