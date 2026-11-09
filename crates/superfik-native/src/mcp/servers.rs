//! Which MCP servers a run gets: the app's own (granted with a token per
//! run) and the plugins' accounts (the MCP config file the backend wrote
//! for the run, `PluginBrief::mcp_config`).

use serde_json::Value;
use superfik_core::PromptRequest;
use superfik_core::browser::BROWSER_SERVER;
use superfik_core::channel::CHANNELS_SERVER;
use superfik_core::memory::MEMORY_SERVER;
use superfik_core::note::NOTES_SERVER;
use superfik_core::vault::VAULT_SERVER;

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

pub fn servers(request: &PromptRequest) -> Vec<ServerSpec> {
    let own = |name: &str, url: &str, token: &str| ServerSpec::Http {
        name: name.to_owned(),
        url: url.to_owned(),
        headers: vec![("authorization".to_owned(), format!("Bearer {token}"))],
    };
    let plugins = request.plugins.as_ref();
    let memory = request.memory.as_ref().and_then(|memory| memory.tool.as_ref());
    let mut servers: Vec<ServerSpec> = memory.map(|t| own(MEMORY_SERVER, &t.url, &t.token)).into_iter().collect();
    servers.extend(plugins.and_then(|p| p.vault.as_ref()).map(|t| own(VAULT_SERVER, &t.url, &t.token)));
    servers.extend(request.browser.as_ref().map(|t| own(BROWSER_SERVER, &t.url, &t.token)));
    servers.extend(plugins.and_then(|p| p.notes.as_ref()).map(|t| own(NOTES_SERVER, &t.url, &t.token)));
    servers.extend(plugins.and_then(|p| p.channels.as_ref()).map(|t| own(CHANNELS_SERVER, &t.url, &t.token)));
    if let Some(path) = plugins.and_then(|p| p.mcp_config.as_ref()) {
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|text| parse_config(&text)) {
            Ok(found) => servers.extend(found.into_iter().filter(|s| !s.name().starts_with("superfik_"))),
            Err(error) => tracing::warn!(%error, "the plugins' MCP servers are not available to this run"),
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
    use superfik_core::{EngineKind, MemoryBrief, MemoryTool, ModelOption, PluginBrief};

    use super::*;

    fn request() -> PromptRequest {
        PromptRequest::plain("/r".into(), "hi", ModelOption::new(EngineKind::Native, "m", "M"))
    }

    #[test]
    fn the_app_s_servers_carry_the_run_s_token() {
        assert!(servers(&request()).is_empty());
        let mut request = request();
        let tool = MemoryTool { url: "http://127.0.0.1:7/mcp".into(), token: "t-1".into() };
        request.memory = Some(MemoryBrief { tool: Some(tool), ..MemoryBrief::default() });
        let found = servers(&request);
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
    fn a_plugin_config_brings_http_and_stdio_servers() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("mcp.json");
        let config = r#"{"mcpServers": {
            "linear-acme": {"type": "http", "url": "https://mcp.linear.app/mcp", "headers": {"Authorization": "Bearer x"}},
            "gmail-work": {"type": "stdio", "command": "npx", "args": ["-y", "gmail-mcp"], "env": {"TOKEN": "y"}},
            "odd": {"type": "websocket", "url": "ws://x"}
        }}"#;
        std::fs::write(&path, config).expect("write");
        let mut request = request();
        request.plugins = Some(PluginBrief { mcp_config: Some(path), ..PluginBrief::default() });
        let found = servers(&request);
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
