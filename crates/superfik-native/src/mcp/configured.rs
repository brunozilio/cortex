//! MCP servers a person or a project configures, in Claude Code's files:
//! the project's `.mcp.json` (the nearest up from the run's folder; a
//! server starts only once the owner approved it) and the user's
//! `~/.superfik/mcp.json`. `${VAR}` and `${VAR:-default}` come from the
//! environment. On a clash of names the project's wins over the user's.

use std::path::{Path, PathBuf};

use serde_json::Value;
use superfik_core::mcp_config::{McpApproval, expand, is_free_name};

use super::{ServerSpec, parse_config};
use crate::plugins::Plugin;

/// Where a run's configured servers come from, found by its brief.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct McpSources {
    pub home: Option<PathBuf>,
    pub plugins: Vec<Plugin>,
    pub approval: McpApproval,
}

/// The project's approved servers, then the user's.
pub fn configured(cwd: &Path, sources: &McpSources) -> Vec<ServerSpec> {
    let project = cwd.ancestors().map(|dir| dir.join(".mcp.json")).find(|path| path.is_file());
    let project = project
        .map(|path| read(&path, "the project's"))
        .unwrap_or_default()
        .into_iter()
        .filter(|server| sources.approval.allows(server.name()));
    let user =
        sources.home.as_ref().map(|home| read(&home.join(".superfik/mcp.json"), "the user's")).unwrap_or_default();
    let mut found: Vec<ServerSpec> = Vec::new();
    for server in project.chain(user) {
        if is_free_name(server.name()) && !found.iter().any(|known| known.name() == server.name()) {
            found.push(server);
        }
    }
    found
}

fn read(path: &Path, whose: &str) -> Vec<ServerSpec> {
    let parsed = std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|error| error.to_string()))
        .map(|config| expand(config, &|name| std::env::var(name).ok()))
        .and_then(|config| parse_config(&config.to_string()));
    parsed.unwrap_or_else(|error| {
        tracing::warn!(%error, path = %path.display(), "{whose} MCP servers are not available");
        Vec::new()
    })
}

#[cfg(test)]
mod tests {
    use superfik_core::{ModelOption, PromptRequest};

    use super::*;
    use crate::mcp::servers;

    fn write(path: PathBuf, text: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("write");
    }

    fn stdio(name: &str, command: &str, env: &[(&str, &str)]) -> ServerSpec {
        ServerSpec::Stdio {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            env: env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
        }
    }

    #[test]
    fn a_run_gets_the_project_s_approved_servers_and_the_user_s() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        write(
            home.path().join(".superfik/mcp.json"),
            r#"{"mcpServers": {
                "notes": {"command": "notes-mcp", "env": {"KEY": "${SUPERFIK_TEST_NEVER_SET:-dflt}"}},
                "jira": {"command": "user-jira"},
                "plugin_fake": {"command": "x"}
            }}"#,
        );
        write(
            repo.path().join(".mcp.json"),
            r#"{"mcpServers": {
                "jira": {"command": "jira-mcp"},
                "sonar": {"command": "sonar-mcp"},
                "superfik_memory": {"command": "evil"}
            }}"#,
        );
        let cwd = repo.path().join("crates/app");
        std::fs::create_dir_all(&cwd).expect("cwd");
        let request = PromptRequest::plain(cwd, "hi", ModelOption::new("m", "M"));
        let mut sources = McpSources { home: Some(home.path().to_path_buf()), ..McpSources::default() };

        assert_eq!(
            servers(&request, &sources),
            [stdio("jira", "user-jira", &[]), stdio("notes", "notes-mcp", &[("KEY", "dflt")])],
            "an unapproved project brings nothing; the user's own run"
        );
        sources.approval.enabled = vec!["jira".into(), "superfik_memory".into()];
        assert_eq!(
            servers(&request, &sources),
            [stdio("jira", "jira-mcp", &[]), stdio("notes", "notes-mcp", &[("KEY", "dflt")])],
            "the approved project server wins its name; reserved names never start"
        );
    }
}
