//! A plugin's manifest: `superfik-plugin.json` at the root of its folder.
//!
//! ```json
//! { "id": "linear", "name": "Linear", "version": "1.0.0",
//!   "auth": { "kind": "oauth" },
//!   "mcp": [{ "transport": "http", "name": "linear", "url": "https://mcp.linear.app/mcp",
//!             "headers": { "Authorization": "Bearer {{secret.access_token}}" } }] }
//! ```

pub mod template;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use superfik_core::{Accounts, AuthKind, PluginId, SecretField, plugins::slug};

use crate::error::PluginError;

pub const MANIFEST_FILE: &str = "superfik-plugin.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub accounts: Accounts,
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub mcp: Vec<McpServerSpec>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Auth {
    #[default]
    None,
    ApiKey {
        fields: Vec<SecretField>,
    },
    #[serde(rename = "oauth")]
    OAuth(OAuthSpec),
    Cli {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Qr,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthSpec {
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub authorization_server: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerSpec {
    Stdio {
        name: String,
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Http {
        name: String,
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpServerSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Stdio { name, .. } | Self::Http { name, .. } => name,
        }
    }

    fn texts(&self) -> Vec<&str> {
        match self {
            Self::Stdio { command, args, env, .. } => std::iter::once(command.as_str())
                .chain(args.iter().map(String::as_str))
                .chain(env.values().map(String::as_str))
                .collect(),
            Self::Http { url, headers, .. } => {
                std::iter::once(url.as_str()).chain(headers.values().map(String::as_str)).collect()
            }
        }
    }
}

impl Auth {
    pub fn kind(&self) -> AuthKind {
        match self {
            Self::None => AuthKind::None,
            Self::ApiKey { .. } => AuthKind::ApiKey,
            Self::OAuth(_) => AuthKind::OAuth,
            Self::Cli { .. } => AuthKind::Cli,
            Self::Qr => AuthKind::Qr,
        }
    }

    pub fn secret_keys(&self) -> Vec<&str> {
        match self {
            Self::None | Self::Qr => Vec::new(),
            Self::ApiKey { fields } => fields.iter().map(|field| field.key.as_str()).collect(),
            Self::OAuth(_) => vec!["access_token"],
            Self::Cli { .. } => vec!["token"],
        }
    }

    pub fn fields(&self) -> &[SecretField] {
        match self {
            Self::ApiKey { fields } => fields,
            _ => &[],
        }
    }
}

impl PluginManifest {
    pub fn parse(json: &str) -> Result<Self, PluginError> {
        let manifest: Self = serde_json::from_str(json).map_err(|error| PluginError::Invalid(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), PluginError> {
        let invalid = |message: String| Err(PluginError::Invalid(format!("{}: {message}", self.id.0)));
        if self.id.0.is_empty() || slug(&self.id.0) != self.id.0 {
            return invalid("id must be lowercase letters, digits and `-`".into());
        }
        if self.name.trim().is_empty() {
            return invalid("name is empty".into());
        }
        if self.auth == Auth::None && self.accounts == Accounts::Many {
            return invalid("a plugin without sign-in cannot connect many accounts".into());
        }
        let mut names = BTreeSet::new();
        for server in &self.mcp {
            if server.name().is_empty() || slug(server.name()) != server.name() {
                return invalid(format!("server name `{}` must be a slug", server.name()));
            }
            if !names.insert(server.name()) {
                return invalid(format!("server `{}` is declared twice", server.name()));
            }
        }
        match &self.auth {
            Auth::ApiKey { fields } if fields.is_empty() => return invalid("api_key auth needs fields".into()),
            Auth::OAuth(spec) if spec.resource.is_none() && self.first_http_url().is_none() => {
                return invalid("oauth needs a resource or an HTTP server".into());
            }
            Auth::Cli { program, .. } if program.trim().is_empty() => {
                return invalid("cli auth needs a program".into());
            }
            _ => {}
        }
        let keys = self.auth.secret_keys();
        let texts = self.mcp.iter().flat_map(McpServerSpec::texts).chain(self.instructions.as_deref());
        for text in texts {
            for name in template::references(text)? {
                let known = matches!(name, "account" | "server")
                    || name.strip_prefix("secret.").is_some_and(|key| keys.contains(&key));
                if !known {
                    return invalid(format!("unknown placeholder `{{{{{name}}}}}`"));
                }
            }
        }
        Ok(())
    }

    pub fn first_http_url(&self) -> Option<&str> {
        self.mcp.iter().find_map(|server| match server {
            McpServerSpec::Http { url, .. } => Some(url.as_str()),
            McpServerSpec::Stdio { .. } => None,
        })
    }
}

#[cfg(test)]
mod tests;
