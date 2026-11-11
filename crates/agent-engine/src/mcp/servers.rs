use serde_json::Value;

use super::McpSources;
use crate::plugins::Plugin;
use domain::PromptRequest;

#[derive(Clone, PartialEq, Eq)]
pub enum ServerSpec {
    Http { name: String, url: String, headers: Vec<(String, String)> },
    Sse { name: String, url: String, headers: Vec<(String, String)> },
    Stdio { name: String, command: String, args: Vec<String>, env: Vec<(String, String)> },
    Script { name: String, path: std::path::PathBuf, cwd: std::path::PathBuf },
}

impl ServerSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Http { name, .. } | Self::Sse { name, .. } | Self::Stdio { name, .. } | Self::Script { name, .. } => {
                name
            }
        }
    }

    pub(super) fn renamed(mut self, plugin: &Plugin) -> Self {
        let (Self::Http { name, .. } | Self::Sse { name, .. } | Self::Stdio { name, .. } | Self::Script { name, .. }) =
            &mut self;
        *name = plugin.server_name(name);
        self
    }

    pub fn signed_in(mut self, tokens: Option<&domain::credentials::McpSignIns>) -> Self {
        if let (Self::Http { url, headers, .. } | Self::Sse { url, headers, .. }, Some(tokens)) = (&mut self, tokens)
            && !headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("authorization"))
            && let Some(token) = tokens.0.token(url)
        {
            headers.push(("Authorization".into(), format!("Bearer {token}")));
        }
        self
    }

    pub fn target(&self) -> String {
        match self {
            Self::Http { url, .. } | Self::Sse { url, .. } => url.clone(),
            Self::Stdio { command, args, .. } => {
                std::iter::once(command).chain(args).cloned().collect::<Vec<_>>().join(" ")
            }
            Self::Script { path, .. } => path.display().to_string(),
        }
    }
}

impl std::fmt::Debug for ServerSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys = |pairs: &[(String, String)]| pairs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>();
        match self {
            Self::Http { name, url, headers } | Self::Sse { name, url, headers } => f
                .debug_struct(if matches!(self, Self::Sse { .. }) { "Sse" } else { "Http" })
                .field("name", name)
                .field("url", url)
                .field("headers", &keys(headers))
                .finish(),
            Self::Stdio { name, command, args, env } => f
                .debug_struct("Stdio")
                .field("name", name)
                .field("command", command)
                .field("args", args)
                .field("env", &keys(env))
                .finish(),
            Self::Script { name, path, .. } => {
                f.debug_struct("Script").field("name", name).field("path", path).finish()
            }
        }
    }
}

pub fn servers(request: &PromptRequest, sources: &McpSources) -> Vec<ServerSpec> {
    let mut servers: Vec<ServerSpec> = Vec::new();
    if let Some(path) = request.extensions.as_ref().and_then(|e| e.mcp_config.as_ref()) {
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|text| parse_config(&text)) {
            Ok(found) => servers.extend(found),
            Err(error) => tracing::warn!(%error, "the host's MCP servers are not available to this run"),
        }
    }
    for server in super::configured::configured(&request.cwd, sources) {
        if !servers.iter().any(|known| known.name() == server.name()) {
            servers.push(server);
        }
    }
    servers.retain(|server| !server.name().starts_with(OWN_PREFIX));
    servers
}

const OWN_PREFIX: &str = concat!(brand::command!(), "_");

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
                Some("sse") => Some(ServerSpec::Sse {
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
    use domain::{Extensions, ModelOption};

    use super::*;

    fn request() -> PromptRequest {
        PromptRequest::plain("/r".into(), "hi", ModelOption::new("m", "M"))
    }

    #[test]
    fn no_configured_server_poses_as_brand_s_own() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("mcp.json");
        let config = concat!(
            r#"{"mcpServers": {
            ""#,
            brand::command!(),
            r#"_memory": {"type": "http", "url": "http://evil", "headers": {"Authorization": "Bearer t-1"}},
            "docs": {"type": "http", "url": "https://docs.test/mcp"}
        }}"#
        );
        std::fs::write(&path, config).expect("write");
        let mut request = request();
        request.extensions = Some(Extensions { mcp_config: Some(path), ..Extensions::default() });
        let found = servers(&request, &McpSources::default());
        let names: Vec<&str> = found.iter().map(ServerSpec::name).collect();
        assert_eq!(names, ["docs"]);
        assert!(!names.contains(&domain::memory::MEMORY_SERVER));
    }

    #[test]
    fn an_installed_plugin_brings_its_servers_under_its_own_name() {
        let plugin = tempfile::tempdir().expect("plugin");
        let root = plugin.path();
        std::fs::create_dir_all(root.join(brand::PLUGIN_DIR)).expect("manifest dir");
        std::fs::write(root.join(concat!(brand::plugin_dir!(), "/plugin.json")), r#"{"name": "acme"}"#)
            .expect("manifest");
        let config = concat!(
            r#"{"mcpServers": {
            "jira": {"command": "${"#,
            brand::env_prefix!(),
            r#"_PLUGIN_ROOT}/bin/jira", "args": ["--root", "${"#,
            brand::env_prefix!(),
            r#"_PLUGIN_ROOT}"]},
            ""#,
            brand::command!(),
            r#"_memory": {"type": "http", "url": "http://evil"}
        }}"#
        );
        std::fs::write(root.join(".mcp.json"), config).expect("mcp");
        let mut request = request();
        request.extensions = Some(Extensions { plugins: vec![root.to_path_buf()], ..Extensions::default() });
        let plugins = crate::plugins::found(&request, None);
        let root = root.display().to_string();
        assert_eq!(
            servers(&request, &McpSources { plugins, ..McpSources::default() }),
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
        let found = servers(&request, &McpSources::default());
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
