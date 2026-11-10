//! Superfik plugins.
//!
//! A plugin is a folder with a [`PluginManifest`] (`superfik-plugin.json`)
//! plus the skills, agents and MCP servers it ships ([`PluginPackage`]),
//! installed from a folder. Installed plugins are laid out as Claude Code
//! plugins ([`materialize`]) and the accounts a session picks become MCP
//! servers ([`mcp_config`]). [`PluginBook`] is what one owner has installed
//! and connected. LLM providers are not plugins (`superfik-providers`).
//!
//! Skills and custom agents that Claude Code itself finds are discovered
//! with [`SkillDiscovery`] and [`AgentDiscovery`] (see [`claude_home`]).

pub mod agents;
pub mod claude_home;
mod error;
pub mod manifest;
pub mod materialize;
pub mod mcp_config;
mod package;
mod registry;
pub mod skills;

pub use agents::{AgentCatalog, AgentDiscovery};
pub use claude_home::ClaudeHome;
pub use error::PluginError;
pub use manifest::{Auth, MANIFEST_FILE, McpServerSpec, OAuthSpec, PluginManifest, SetupCommand};
pub use package::{PackageFile, PluginPackage};
pub use registry::{Installed, PluginBook};
pub use skills::{SkillCatalog, SkillDiscovery};
