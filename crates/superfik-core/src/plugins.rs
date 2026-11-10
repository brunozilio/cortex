//! Plugins and the accounts they connect, as the rest of the product sees
//! them. A plugin brings skills, agents and MCP tools; a [`Connection`] is
//! one signed-in account of a plugin (two Gmail accounts are two
//! connections). A session picks its connections the way it picks skills.
//! Secrets never appear here: the backend keeps them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ids::Id;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(pub String);

impl PluginId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub enum ConnectionMarker {}
pub type ConnectionId = Id<ConnectionMarker>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accounts {
    #[default]
    One,
    Many,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    None,
    ApiKey,
    OAuth,
    Cli,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginOrigin {
    Local(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSummary {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub icon: String,
    #[serde(default)]
    pub category: String,
    pub accounts: Accounts,
    pub auth: AuthKind,
    #[serde(default)]
    pub fields: Vec<SecretField>,
    pub origin: PluginOrigin,
    pub installed: bool,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub servers: Vec<String>,
}

impl PluginSummary {
    pub fn needs_account(&self) -> bool {
        self.auth != AuthKind::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub id: ConnectionId,
    pub plugin: PluginId,
    pub plugin_name: String,
    pub label: String,
    pub icon: String,
}

impl Connection {
    pub fn slash_name(&self) -> String {
        let label = slug(&self.label);
        if label.is_empty() || label == self.plugin.0 {
            self.plugin.0.clone()
        } else {
            format!("{}-{label}", self.plugin.0)
        }
    }

    pub fn title(&self) -> String {
        if self.label.is_empty() { self.plugin_name.clone() } else { format!("{} · {}", self.plugin_name, self.label) }
    }
}

pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginBrief {
    #[serde(default)]
    pub mcp_config: Option<PathBuf>,
    #[serde(default)]
    pub plugin_dirs: Vec<PathBuf>,
    /// Folders laid out like a plugin whose skills, commands and agents are
    /// offered under their own names, without a plugin namespace (the
    /// SDK's).
    #[serde(default)]
    pub local_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub instructions: Vec<String>,
}

impl PluginBrief {
    pub fn is_empty(&self) -> bool {
        self.mcp_config.is_none()
            && self.plugin_dirs.is_empty()
            && self.local_dirs.is_empty()
            && self.instructions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(label: &str) -> Connection {
        Connection {
            id: ConnectionId::new(),
            plugin: PluginId("gmail".into()),
            plugin_name: "Gmail".into(),
            label: label.into(),
            icon: "mail".into(),
        }
    }

    #[test]
    fn slugs_keep_letters_and_digits() {
        assert_eq!(slug("Bruno @ Acme.com"), "bruno-acme-com");
        assert_eq!(slug("  Work  "), "work");
        assert_eq!(slug("!!"), "");
    }

    #[test]
    fn connections_are_named_by_plugin_and_account() {
        assert_eq!(connection("Work").slash_name(), "gmail-work");
        assert_eq!(connection("").slash_name(), "gmail");
        assert_eq!(connection("gmail").slash_name(), "gmail");
        assert_eq!(connection("Work").title(), "Gmail · Work");
        assert_eq!(connection("").title(), "Gmail");
    }
}
