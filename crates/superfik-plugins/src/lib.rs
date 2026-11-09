//! Superfik plugins.
//!
//! A plugin is a folder with a [`PluginManifest`] (`superfik-plugin.json`)
//! plus the skills and agents it ships ([`PluginPackage`]); the
//! [`catalog`] embeds the curated ones. Installed plugins are laid out as
//! Claude Code plugins ([`materialize`]) and the accounts a session picks
//! become MCP servers ([`mcp_config`]). [`PluginBook`] is what one owner
//! has installed and connected. The AI provider plugins' credentials,
//! sign-ins, catalogs and request shapes are in [`providers`].
//!
//! Skills and custom agents that Claude Code itself finds are discovered
//! with [`SkillDiscovery`] and [`AgentDiscovery`] (see [`claude_home`]).

pub mod agents;
pub mod catalog;
pub mod claude_home;
mod error;
pub mod library;
pub mod manifest;
pub mod materialize;
pub mod mcp_config;
mod package;
pub mod providers;
mod registry;
pub mod skills;

pub use agents::{AgentCatalog, AgentDiscovery};
pub use claude_home::ClaudeHome;
pub use error::PluginError;
pub use manifest::{Auth, MANIFEST_FILE, McpServerSpec, OAuthSpec, PluginManifest, SetupCommand};
pub use package::{PackageFile, PluginPackage};
pub use registry::{Installed, PluginBook};
pub use skills::{SkillCatalog, SkillDiscovery};
