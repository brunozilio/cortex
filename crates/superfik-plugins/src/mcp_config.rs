//! The MCP configuration a run gets for the accounts its session picked.
//! Each account gets its own server name (`gmail-work`), so two accounts
//! of one plugin expose distinct tools (`mcp__gmail-work__send`). Pure:
//! the backend supplies the secrets and writes the file.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use superfik_core::Connection;

use crate::error::PluginError;
use crate::manifest::template::{Vars, render};
use crate::manifest::{McpServerSpec, PluginManifest};

#[derive(Debug, Clone, Copy)]
pub struct Account<'a> {
    pub manifest: &'a PluginManifest,
    pub connection: &'a Connection,
    pub secrets: &'a BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpPlan {
    pub config: Value,
    pub instructions: Vec<String>,
}

impl McpPlan {
    pub fn is_empty(&self) -> bool {
        self.config["mcpServers"].as_object().is_none_or(Map::is_empty)
    }
}

pub fn plan(accounts: &[Account]) -> Result<McpPlan, PluginError> {
    let mut servers = Map::new();
    let mut instructions = Vec::new();
    for account in accounts {
        let base = unique(&servers, &account.connection.slash_name());
        let mut names = Vec::new();
        for spec in &account.manifest.mcp {
            let name = if account.manifest.mcp.len() == 1 { base.clone() } else { format!("{base}-{}", spec.name()) };
            let vars = Vars { secrets: account.secrets, account: &account.connection.label, server: &name };
            servers.insert(name.clone(), server(spec, vars)?);
            names.push(name);
        }
        let Some(first) = names.first() else { continue };
        let vars = Vars { secrets: account.secrets, account: &account.connection.label, server: first };
        let line = match &account.manifest.instructions {
            Some(text) => format!("{}: {}", account.connection.title(), render(text, vars)?),
            None => {
                let tools: Vec<String> = names.iter().map(|name| format!("mcp__{name}__*")).collect();
                format!("{}: tools {}", account.connection.title(), tools.join(", "))
            }
        };
        instructions.push(line);
    }
    Ok(McpPlan { config: json!({ "mcpServers": servers }), instructions })
}

fn unique(servers: &Map<String, Value>, name: &str) -> String {
    let taken =
        |candidate: &str| servers.keys().any(|key| key == candidate || key.starts_with(&format!("{candidate}-")));
    if !taken(name) {
        return name.to_owned();
    }
    (2..).map(|n| format!("{name}-{n}")).find(|candidate| !taken(candidate)).unwrap_or_default()
}

/// A server the host adds to every run, written as given (no templates).
pub fn plain(spec: &McpServerSpec) -> Value {
    match spec {
        McpServerSpec::Stdio { command, args, env, .. } => {
            json!({ "type": "stdio", "command": command, "args": args, "env": env })
        }
        McpServerSpec::Http { url, headers, .. } => json!({ "type": "http", "url": url, "headers": headers }),
    }
}

fn server(spec: &McpServerSpec, vars: Vars) -> Result<Value, PluginError> {
    let map = |values: &BTreeMap<String, String>| -> Result<Map<String, Value>, PluginError> {
        values.iter().map(|(key, value)| Ok((key.clone(), Value::String(render(value, vars)?)))).collect()
    };
    Ok(match spec {
        McpServerSpec::Stdio { command, args, env, .. } => json!({
            "type": "stdio",
            "command": render(command, vars)?,
            "args": args.iter().map(|arg| render(arg, vars)).collect::<Result<Vec<_>, _>>()?,
            "env": map(env)?,
        }),
        McpServerSpec::Http { url, headers, .. } => json!({
            "type": "http",
            "url": render(url, vars)?,
            "headers": map(headers)?,
        }),
    })
}

#[cfg(test)]
mod tests {
    use superfik_core::{ConnectionId, PluginId};

    use super::*;

    fn gmail() -> PluginManifest {
        PluginManifest::parse(
            r#"{"id":"gmail","name":"Gmail","version":"1","accounts":"many","auth":{"kind":"oauth"},
            "mcp":[{"transport":"http","name":"gmail","url":"https://g/mcp",
                    "headers":{"Authorization":"Bearer {{secret.access_token}}"}}],
            "instructions":"mail of {{account}} through mcp__{{server}}__*"}"#,
        )
        .expect("manifest")
    }

    fn connection(label: &str) -> Connection {
        Connection {
            id: ConnectionId::new(),
            plugin: PluginId("gmail".into()),
            plugin_name: "Gmail".into(),
            label: label.into(),
            icon: String::new(),
        }
    }

    fn token(value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("access_token".to_owned(), value.to_owned())])
    }

    #[test]
    fn two_accounts_get_two_servers() {
        let manifest = gmail();
        let (work, home) = (connection("Work"), connection("Home"));
        let (t1, t2) = (token("t-work"), token("t-home"));
        let plan = plan(&[
            Account { manifest: &manifest, connection: &work, secrets: &t1 },
            Account { manifest: &manifest, connection: &home, secrets: &t2 },
        ])
        .expect("plan");
        let servers = &plan.config["mcpServers"];
        assert_eq!(servers["gmail-work"]["type"], "http");
        assert_eq!(servers["gmail-work"]["headers"]["Authorization"], "Bearer t-work");
        assert_eq!(servers["gmail-home"]["headers"]["Authorization"], "Bearer t-home");
        assert_eq!(
            plan.instructions,
            vec![
                "Gmail · Work: mail of Work through mcp__gmail-work__*",
                "Gmail · Home: mail of Home through mcp__gmail-home__*",
            ]
        );
    }

    #[test]
    fn same_slug_is_made_unique_and_stdio_is_rendered() {
        let manifest = PluginManifest::parse(
            r#"{"id":"pg","name":"Postgres","version":"1","accounts":"many",
            "auth":{"kind":"api_key","fields":[{"key":"url","label":"URL"}]},
            "mcp":[{"transport":"stdio","name":"pg","command":"npx","args":["-y","pg-mcp","{{secret.url}}"]}]}"#,
        )
        .expect("manifest");
        let (a, b) = (connection("db"), connection("DB"));
        let secrets = BTreeMap::from([("url".to_owned(), "postgres://x".to_owned())]);
        let plan = plan(&[
            Account { manifest: &manifest, connection: &a, secrets: &secrets },
            Account { manifest: &manifest, connection: &b, secrets: &secrets },
        ])
        .expect("plan");
        let servers = plan.config["mcpServers"].as_object().expect("servers");
        assert_eq!(servers.keys().collect::<Vec<_>>(), vec!["gmail-db", "gmail-db-2"]);
        assert_eq!(servers["gmail-db"]["args"][2], "postgres://x");
        assert_eq!(plan.instructions[0], "Gmail · db: tools mcp__gmail-db__*");
    }

    #[test]
    fn a_missing_secret_fails_and_nothing_is_empty() {
        let manifest = gmail();
        let work = connection("Work");
        let none = BTreeMap::new();
        assert!(plan(&[Account { manifest: &manifest, connection: &work, secrets: &none }]).is_err());
        assert!(plan(&[]).expect("plan").is_empty());
    }
}
