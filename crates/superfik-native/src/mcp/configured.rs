//! MCP servers a person or a project configures, in Claude Code's files:
//! the project's `.mcp.json` (the nearest up from the run's folder; a
//! server starts only once the owner approved it) and the user's
//! `~/.superfik/mcp.json`. `${VAR}` and `${VAR:-default}` come from the
//! environment. On a clash of names the project's wins over the user's.
//! Installed plugins' servers follow, as `plugin_<plugin>_<server>`.

use std::path::{Path, PathBuf};

use serde_json::Value;
use superfik_core::PromptRequest;
use superfik_core::mcp_config::{McpApproval, McpOrigin, McpServerInfo, McpState, expand, is_free_name};

use super::{ServerSpec, parse_config};
use crate::plugins::Plugin;

/// Where a run's configured servers come from, found by its brief.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct McpSources {
    pub home: Option<PathBuf>,
    pub plugins: Vec<Plugin>,
    pub approval: McpApproval,
}

/// The servers that start with a run: every configured one that is ready.
pub fn configured(cwd: &Path, sources: &McpSources) -> Vec<ServerSpec> {
    origins(cwd, sources)
        .into_iter()
        .filter(|(origin, server)| McpState::of(origin, server.name(), &sources.approval) == McpState::Ready)
        .map(|(_, server)| server)
        .collect()
}

/// Every configured server, where it came from and whether it starts.
pub fn list(request: &PromptRequest) -> Result<Vec<McpServerInfo>, String> {
    let sources = crate::brief::mcp_sources(request)?;
    Ok(origins(&request.cwd, &sources)
        .into_iter()
        .map(|(origin, server)| McpServerInfo {
            name: server.name().to_owned(),
            state: McpState::of(&origin, server.name(), &sources.approval),
            target: server.target(),
            origin,
        })
        .collect())
}

fn origins(cwd: &Path, sources: &McpSources) -> Vec<(McpOrigin, ServerSpec)> {
    let project = cwd.ancestors().map(|dir| dir.join(".mcp.json")).find(|path| path.is_file());
    let project = project.map(|path| read(&path, "the project's")).unwrap_or_default();
    let user =
        sources.home.as_ref().map(|home| read(&home.join(".superfik/mcp.json"), "the user's")).unwrap_or_default();
    let mut found: Vec<(McpOrigin, ServerSpec)> = Vec::new();
    let own =
        project.into_iter().map(|s| (McpOrigin::Project, s)).chain(user.into_iter().map(|s| (McpOrigin::User, s)));
    // Only a server that starts takes its name: a project's server nobody
    // approved never shadows the person's own.
    for (origin, server) in own {
        let taken = found.iter().any(|(known_origin, known)| {
            known.name() == server.name()
                && McpState::of(known_origin, known.name(), &sources.approval) == McpState::Ready
        });
        if is_free_name(server.name()) && !taken {
            found.push((origin, server));
        }
    }
    for plugin in &sources.plugins {
        let origin = McpOrigin::Plugin(plugin.name.clone());
        found.extend(plugin_servers(plugin).into_iter().map(|server| (origin.clone(), server)));
    }
    found
}

fn plugin_servers(plugin: &Plugin) -> Vec<ServerSpec> {
    let Some(config) = plugin.mcp_config() else { return Vec::new() };
    match parse_config(&config.to_string()) {
        Ok(found) => {
            found.into_iter().filter(|s| !s.name().starts_with("superfik_")).map(|s| s.renamed(plugin)).collect()
        }
        Err(error) => {
            tracing::warn!(%error, plugin = %plugin.name, "a plugin's MCP servers are not available");
            Vec::new()
        }
    }
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
    fn the_listing_shows_every_server_its_origin_and_whether_it_starts() {
        use superfik_core::Extensions;
        use superfik_core::mcp_config::{McpOrigin, McpServerInfo, McpState};
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let plugin = tempfile::tempdir().expect("plugin");
        write(
            home.path().join(".superfik/mcp.json"),
            r#"{"mcpServers": {"atlassian": {"type": "http", "url": "https://mcp.atlassian.com/v1/mcp", "headers": {"Authorization": "Bearer secret"}}}}"#,
        );
        write(
            repo.path().join(".mcp.json"),
            r#"{"mcpServers": {"jira": {"command": "jira-mcp", "args": ["--stdio"]}, "sonar": {"command": "sonar-mcp"}}}"#,
        );
        write(repo.path().join(".superfik/settings.local.json"), r#"{"disabledMcpjsonServers": ["sonar"]}"#);
        write(plugin.path().join(".superfik-plugin/plugin.json"), r#"{"name": "acme"}"#);
        write(plugin.path().join(".mcp.json"), r#"{"mcpServers": {"datadog": {"command": "dd-mcp"}}}"#);
        let mut request = PromptRequest::plain(repo.path().to_path_buf(), "", ModelOption::new("", ""));
        request.extensions = Some(Extensions {
            home: Some(home.path().to_path_buf()),
            plugins: vec![plugin.path().to_path_buf()],
            ..Extensions::default()
        });
        let info = |name: &str, origin: McpOrigin, target: &str, state: McpState| McpServerInfo {
            name: name.into(),
            origin,
            target: target.into(),
            state,
        };
        let listed = crate::mcp::list(&request).expect("listed");
        assert_eq!(
            listed,
            [
                info("jira", McpOrigin::Project, "jira-mcp --stdio", McpState::Pending),
                info("sonar", McpOrigin::Project, "sonar-mcp", McpState::Off),
                info("atlassian", McpOrigin::User, "https://mcp.atlassian.com/v1/mcp", McpState::Ready),
                info("plugin_acme_datadog", McpOrigin::Plugin("acme".into()), "dd-mcp", McpState::Ready),
            ]
        );
        assert!(!format!("{listed:?}").contains("secret"), "headers stay out");
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
