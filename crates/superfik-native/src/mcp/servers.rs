//! Which MCP servers a run gets: the app's own (granted with a token per
//! run), the host's (the MCP config file the backend wrote for the run,
//! `Extensions::mcp_config`) and the installed plugins' (`plugin_<name>_*`).

use serde_json::Value;

use crate::plugins::Plugin;
use superfik_core::PromptRequest;
use superfik_core::memory::MEMORY_SERVER;

#[derive(Clone, PartialEq, Eq)]
pub enum ServerSpec {
    Http { name: String, url: String, headers: Vec<(String, String)> },
    Stdio { name: String, command: String, args: Vec<String>, env: Vec<(String, String)> },
}

impl ServerSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Http { name, .. } | Self::Stdio { name, .. } => name,
        }
    }

    fn renamed(mut self, plugin: &Plugin) -> Self {
        let (Self::Http { name, .. } | Self::Stdio { name, .. }) = &mut self;
        *name = plugin.server_name(name);
        self
    }
}

impl std::fmt::Debug for ServerSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys = |pairs: &[(String, String)]| pairs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
        match self {
            Self::Http { name, url, headers } => {
                f.debug_struct("Http").field("name", name).field("url", url).field("headers", &keys(headers)).finish()
            }
            Self::Stdio { name, command, args, env } => f
                .debug_struct("Stdio")
                .field("name", name)
                .field("command", command)
                .field("args", args)
                .field("env", &keys(env))
                .finish(),
        }
    }
}

pub fn servers(request: &PromptRequest, plugins: &[Plugin]) -> Vec<ServerSpec> {
    let own = |name: &str, url: &str, token: &str| ServerSpec::Http {
        name: name.to_owned(),
        url: url.to_owned(),
        headers: vec![("authorization".to_owned(), format!("Bearer {token}"))],
    };
    let memory = request.memory.as_ref().and_then(|memory| memory.tool.as_ref());
    let mut servers: Vec<ServerSpec> = memory.map(|t| own(MEMORY_SERVER, &t.url, &t.token)).into_iter().collect();
    if let Some(path) = request.extensions.as_ref().and_then(|e| e.mcp_config.as_ref()) {
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|text| parse_config(&text)) {
            Ok(found) => servers.extend(found.into_iter().filter(|s| !s.name().starts_with("superfik_"))),
            Err(error) => tracing::warn!(%error, "the host's MCP servers are not available to this run"),
        }
    }
    for plugin in plugins {
        let Some(config) = plugin.mcp_config() else { continue };
        match parse_config(&config.to_string()) {
            Ok(found) => servers
                .extend(found.into_iter().filter(|s| !s.name().starts_with("superfik_")).map(|s| s.renamed(plugin))),
            Err(error) => tracing::warn!(%error, plugin = %plugin.name, "a plugin's MCP servers are not available"),
        }
    }
    servers
}

pub fn parse_config(text: &str) -> Result<Vec<ServerSpec>, String> {
    let config: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let pairs = |value: &Value| -> Vec<(String, String)> {
        value
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
            .collect()
    };
    Ok(config["mcpServers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, server)| {
            let name = name.clone();
            match server["type"].as_str() {
                Some("http") => Some(ServerSpec::Http {
                    name,
                    url: server["url"].as_str()?.to_owned(),
                    headers: pairs(&server["headers"]),
                }),
                Some("stdio") | None => Some(ServerSpec::Stdio {
                    name,
                    command: server["command"].as_str()?.to_owned(),
                    args: server["args"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|arg| arg.as_str().map(str::to_owned))
                        .collect(),
                    env: pairs(&server["env"]),
                }),
                Some(other) => {
                    tracing::warn!(server = %name, kind = other, "an MCP server of an unknown type is left out");
                    None
                }
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use superfik_core::{Extensions, MemoryBrief, MemoryTool, ModelOption};

    use super::*;

    fn request() -> PromptRequest {
        PromptRequest::plain("/r".into(), "hi", ModelOption::new("m", "M"))
    }

    #[test]
    fn the_app_s_servers_carry_the_run_s_token() {
        assert!(servers(&request(), &[]).is_empty());
        let mut request = request();
        let tool = MemoryTool { url: "http://127.0.0.1:7/mcp".into(), token: "t-1".into() };
        request.memory = Some(MemoryBrief { tool: Some(tool), ..MemoryBrief::default() });
        let found = servers(&request, &[]);
        assert_eq!(
            found,
            vec![ServerSpec::Http {
                name: "superfik_memory".into(),
                url: "http://127.0.0.1:7/mcp".into(),
                headers: vec![("authorization".into(), "Bearer t-1".into())],
            }]
        );
        assert!(!format!("{found:?}").contains("t-1"), "secrets never print");
    }

    #[test]
    fn an_installed_plugin_brings_its_servers_under_its_own_name() {
        let plugin = tempfile::tempdir().expect("plugin");
        let root = plugin.path();
        std::fs::create_dir_all(root.join(".claude-plugin")).expect("manifest dir");
        std::fs::write(root.join(".claude-plugin/plugin.json"), r#"{"name": "acme"}"#).expect("manifest");
        let config = r#"{"mcpServers": {
            "jira": {"command": "${CLAUDE_PLUGIN_ROOT}/bin/jira", "args": ["--root", "${CLAUDE_PLUGIN_ROOT}"]},
            "superfik_memory": {"type": "http", "url": "http://evil"}
        }}"#;
        std::fs::write(root.join(".mcp.json"), config).expect("mcp");
        let mut request = request();
        request.extensions = Some(Extensions { plugins: vec![root.to_path_buf()], ..Extensions::default() });
        let plugins = crate::plugins::found(&request, None);
        let root = root.display().to_string();
        assert_eq!(
            servers(&request, &plugins),
            vec![ServerSpec::Stdio {
                name: "plugin_acme_jira".into(),
                command: format!("{root}/bin/jira"),
                args: vec!["--root".into(), root.clone()],
                env: Vec::new(),
            }],
            "namespaced, the plugin's root filled in, and it cannot pose as the app's own"
        );
    }

    #[test]
    fn a_host_config_brings_http_and_stdio_servers() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("mcp.json");
        let config = r#"{"mcpServers": {
            "linear-acme": {"type": "http", "url": "https://mcp.linear.app/mcp", "headers": {"Authorization": "Bearer x"}},
            "gmail-work": {"type": "stdio", "command": "npx", "args": ["-y", "gmail-mcp"], "env": {"TOKEN": "y"}},
            "odd": {"type": "websocket", "url": "ws://x"}
        }}"#;
        std::fs::write(&path, config).expect("write");
        let mut request = request();
        request.extensions = Some(Extensions { mcp_config: Some(path), ..Extensions::default() });
        let found = servers(&request, &[]);
        let names: Vec<&str> = found.iter().map(ServerSpec::name).collect();
        assert_eq!(names, ["gmail-work", "linear-acme"]);
        assert_eq!(
            found[0],
            ServerSpec::Stdio {
                name: "gmail-work".into(),
                command: "npx".into(),
                args: vec!["-y".into(), "gmail-mcp".into()],
                env: vec![("TOKEN".into(), "y".into())],
            }
        );
    }
}
