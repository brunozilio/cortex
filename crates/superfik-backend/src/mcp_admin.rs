//! The owner's MCP servers: what a run in a folder can use (the host's,
//! the project's, the owner's, installed plugins'), approving a project's
//! server or turning it off (the project's `.superfik/settings.local.json`),
//! and adding or removing the owner's own (`~/.superfik/mcp.json`). The
//! rules are `superfik_core::mcp_config`'s; this is their IO. Both files
//! are written readable only by the owner: they may hold tokens.

use std::path::{Path, PathBuf};

use serde_json::Value;
use superfik_core::mcp_config::{self, McpOrigin, McpServerInfo, McpState};
use superfik_core::{ModelOption, PromptRequest};
use superfik_protocol::ServerEvent;

use crate::extensions::{Extensions, McpServerSpec};

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> ServerEvent {
    let mut request = PromptRequest::plain(path.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(superfik_core::Extensions {
        plugins: extensions.plugins.clone(),
        home: home.map(Path::to_path_buf),
        ..Default::default()
    });
    let host = extensions.mcp.iter().map(|spec| McpServerInfo {
        name: spec.name().to_owned(),
        origin: McpOrigin::Host,
        target: match spec {
            McpServerSpec::Http { url, .. } => url.clone(),
            McpServerSpec::Stdio { command, args, .. } => {
                std::iter::once(command).chain(args).cloned().collect::<Vec<_>>().join(" ")
            }
        },
        state: McpState::Ready,
    });
    match superfik_native::mcp::list(&request) {
        Ok(found) => ServerEvent::McpServers { path: path.to_path_buf(), servers: host.chain(found).collect() },
        Err(error) => ServerEvent::error(format!("Could not read the MCP servers: {error}")),
    }
}

pub(crate) fn approve(path: &Path, name: &str, approve: bool) -> Result<(), String> {
    let project = path.ancestors().find(|dir| dir.join(".mcp.json").is_file()).unwrap_or(path);
    let file = project.join(".superfik").join("settings.local.json");
    rewrite(&file, |settings| Ok(mcp_config::set_approval(settings, name, approve)))
}

pub(crate) fn add(home: Option<&Path>, name: &str, server: Value) -> Result<(), String> {
    rewrite(&user_config(home)?, |config| mcp_config::add_server(config, name, server))
}

pub(crate) fn remove(home: Option<&Path>, name: &str) -> Result<(), String> {
    rewrite(&user_config(home)?, |config| match mcp_config::remove_server(config, name) {
        (config, true) => Ok(config),
        (_, false) => Err(format!("`{name}` is not one of your MCP servers")),
    })
}

fn user_config(home: Option<&Path>) -> Result<PathBuf, String> {
    home.map(|home| home.join(".superfik").join("mcp.json")).ok_or_else(|| "no home folder to keep it in".to_owned())
}

fn rewrite(file: &Path, change: impl FnOnce(Value) -> Result<Value, String>) -> Result<(), String> {
    let current = match std::fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| format!("{}: {error}", file.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Value::Null,
        Err(error) => return Err(format!("{}: {error}", file.display())),
    };
    let changed = change(current)?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(&changed).map_err(|error| error.to_string())?;
    superfik_private_file::write(file, text.as_bytes()).map_err(|error| format!("{}: {error}", file.display()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn states(event: ServerEvent) -> Vec<(String, McpOrigin, McpState)> {
        let ServerEvent::McpServers { servers, .. } = event else { panic!("not a listing: {event:?}") };
        servers.into_iter().map(|s| (s.name, s.origin, s.state)).collect()
    }

    #[test]
    fn the_owner_approves_a_project_s_server_and_keeps_their_own() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::write(repo.path().join(".mcp.json"), r#"{"mcpServers": {"jira": {"command": "jira-mcp"}}}"#)
            .expect("config");
        let cwd = repo.path().join("src");
        std::fs::create_dir_all(&cwd).expect("cwd");
        let host = Extensions {
            mcp: vec![McpServerSpec::Http {
                name: "docs".into(),
                url: "https://docs.test/mcp".into(),
                headers: Default::default(),
            }],
            ..Extensions::default()
        };
        let list = || states(list(&cwd, &host, Some(home.path())));
        assert_eq!(
            list(),
            [("docs".into(), McpOrigin::Host, McpState::Ready), ("jira".into(), McpOrigin::Project, McpState::Pending)]
        );

        approve(&cwd, "jira", true).expect("approve");
        assert!(repo.path().join(".superfik/settings.local.json").is_file(), "kept beside the project's .mcp.json");
        assert_eq!(list()[1].2, McpState::Ready);

        let atlassian = json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"});
        add(Some(home.path()), "atlassian", atlassian).expect("add");
        assert_eq!(list()[2], ("atlassian".into(), McpOrigin::User, McpState::Ready));
        assert!(add(Some(home.path()), "superfik_memory", json!({"command": "x"})).is_err());

        remove(Some(home.path()), "atlassian").expect("remove");
        assert_eq!(list().len(), 2);
        assert!(remove(Some(home.path()), "atlassian").is_err(), "nothing left to remove");
    }
}
