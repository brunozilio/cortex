//! Installed plugins, in Claude Code's plugin layout: a folder with
//! `.claude-plugin/plugin.json`, `skills/`, `agents/`, `commands/`,
//! `hooks/hooks.json` and `.mcp.json`. They come from the backend
//! (`Extensions::plugins`) and from Claude Code's own installs; on a clash
//! of names the backend's wins. `${CLAUDE_PLUGIN_ROOT}` in a plugin's hooks
//! and MCP servers is its folder.

use std::path::{Path, PathBuf};

use serde_json::Value;
use superfik_core::PromptRequest;

use crate::claude_home::ClaudeHome;

const ROOT_VARIABLE: &str = "${CLAUDE_PLUGIN_ROOT}";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plugin {
    pub name: String,
    pub root: PathBuf,
}

/// The plugins a run in `request.cwd` gets.
pub fn found(request: &PromptRequest, home: Option<&Path>) -> Vec<Plugin> {
    let hosted = request.extensions.iter().flat_map(|extensions| &extensions.plugins).map(|root| Plugin::at(root));
    let claude = home
        .map(|home| ClaudeHome::new(home.join(".claude")).installed_plugins(&request.cwd))
        .unwrap_or_default()
        .into_iter()
        .map(|installed| Plugin { name: installed.name, root: installed.path });
    let mut found: Vec<Plugin> = Vec::new();
    for plugin in hosted.chain(claude) {
        if !found.iter().any(|known| known.name == plugin.name) {
            found.push(plugin);
        }
    }
    found
}

impl Plugin {
    /// A plugin's folder, named by its manifest or else by the folder.
    pub fn at(root: &Path) -> Self {
        let manifest = read_json(&root.join(".claude-plugin").join("plugin.json"));
        let name = manifest["name"]
            .as_str()
            .filter(|name| !name.trim().is_empty())
            .map(str::to_owned)
            .or_else(|| root.file_name().map(|name| name.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "plugin".to_owned());
        Self { name, root: root.to_path_buf() }
    }

    /// The plugin's `mcpServers` (`.mcp.json`, else the manifest's), its
    /// root filled in.
    pub fn mcp_config(&self) -> Option<Value> {
        let own = read_json(&self.root.join(".mcp.json"));
        let config = if own["mcpServers"].is_object() {
            own
        } else {
            let manifest = read_json(&self.root.join(".claude-plugin").join("plugin.json"));
            if !manifest["mcpServers"].is_object() {
                return None;
            }
            serde_json::json!({ "mcpServers": manifest["mcpServers"] })
        };
        Some(self.expand(config))
    }

    /// The plugin's `hooks/hooks.json` events, its root filled in.
    pub fn hooks(&self) -> Result<Option<Value>, String> {
        let path = self.root.join("hooks").join("hooks.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let value: Value = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let events = match value.get("hooks") {
            Some(events) => events.clone(),
            None => value,
        };
        Ok(Some(self.expand(events)))
    }

    /// A name an MCP server of this plugin can carry: `plugin_<plugin>_<server>`.
    pub fn server_name(&self, server: &str) -> String {
        let clean = |text: &str| -> String {
            text.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
        };
        format!("plugin_{}_{}", clean(&self.name), clean(server))
    }

    fn expand(&self, value: Value) -> Value {
        let root = self.root.display().to_string();
        match value {
            Value::String(text) => Value::String(text.replace(ROOT_VARIABLE, &root)),
            Value::Array(items) => Value::Array(items.into_iter().map(|item| self.expand(item)).collect()),
            Value::Object(map) => Value::Object(map.into_iter().map(|(key, item)| (key, self.expand(item))).collect()),
            other => other,
        }
    }
}

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Value::Null)
}
