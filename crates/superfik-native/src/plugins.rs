//! Superfik plugins (ADR 0019): a folder with `.superfik-plugin/plugin.json`,
//! `skills/`, `agents/`, `commands/`, `hooks/hooks.json` and `.mcp.json`.
//! They come from the backend (`Extensions::plugins`), then from the
//! folders a person puts in `~/.superfik/plugins/`; on a clash of names the
//! first wins. `${SUPERFIK_PLUGIN_ROOT}` in a plugin's hooks and MCP servers
//! is its folder.

use std::path::{Path, PathBuf};

use serde_json::Value;
use superfik_core::PromptRequest;

const MANIFEST_DIR: &str = ".superfik-plugin";
const ROOT_VARIABLE: &str = "${SUPERFIK_PLUGIN_ROOT}";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plugin {
    pub name: String,
    pub root: PathBuf,
}

/// The plugins a run in `request.cwd` gets.
pub fn found(request: &PromptRequest, home: Option<&Path>) -> Vec<Plugin> {
    let hosted = request.extensions.iter().flat_map(|extensions| &extensions.plugins).map(|root| Plugin::at(root));
    let own = home
        .map(|home| subdirs(&home.join(".superfik").join("plugins")))
        .unwrap_or_default()
        .into_iter()
        .map(|root| Plugin::at(&root));
    let mut found: Vec<Plugin> = Vec::new();
    for plugin in hosted.chain(own) {
        if !found.iter().any(|known| known.name == plugin.name) {
            found.push(plugin);
        }
    }
    found
}

impl Plugin {
    /// A plugin's folder, named by its manifest or else by the folder.
    pub fn at(root: &Path) -> Self {
        let manifest = read_json(&root.join(MANIFEST_DIR).join("plugin.json"));
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
            let manifest = read_json(&self.root.join(MANIFEST_DIR).join("plugin.json"));
            if !manifest["mcpServers"].is_object() {
                return None;
            }
            serde_json::json!({ "mcpServers": manifest["mcpServers"] })
        };
        Some(self.expand(config))
    }

    /// The plugin's language servers (`.lsp.json`, else the manifest's
    /// `lspServers`), its root filled in.
    pub fn lsp_servers(&self) -> Option<Value> {
        let own = read_json(&self.root.join(".lsp.json"));
        let servers = if own.is_object() {
            own
        } else {
            let manifest = read_json(&self.root.join(MANIFEST_DIR).join("plugin.json"));
            manifest.get("lspServers").filter(|servers| servers.is_object())?.clone()
        };
        Some(self.expand(servers))
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

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    dirs.sort();
    dirs
}

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use superfik_core::{Extensions, ModelOption};

    use super::*;

    #[test]
    fn a_folder_in_the_user_s_plugins_is_a_plugin_named_by_its_manifest_or_folder() {
        let home = tempfile::tempdir().expect("home");
        let plugins = home.path().join(".superfik/plugins");
        std::fs::create_dir_all(plugins.join("acme-tools/.superfik-plugin")).expect("acme");
        std::fs::write(plugins.join("acme-tools/.superfik-plugin/plugin.json"), r#"{"name": "acme"}"#)
            .expect("manifest");
        std::fs::create_dir_all(plugins.join("beta")).expect("beta");
        std::fs::write(plugins.join("notes.txt"), "not a plugin").expect("file");
        let hosted = tempfile::tempdir().expect("hosted");
        std::fs::create_dir_all(hosted.path().join(".superfik-plugin")).expect("dir");
        std::fs::write(hosted.path().join(".superfik-plugin/plugin.json"), r#"{"name": "beta"}"#).expect("manifest");
        let mut request = PromptRequest::plain("/repo".into(), "", ModelOption::new("", ""));
        request.extensions = Some(Extensions { plugins: vec![hosted.path().to_path_buf()], ..Extensions::default() });

        let found = found(&request, Some(home.path()));
        let named: Vec<(&str, &Path)> = found.iter().map(|p| (p.name.as_str(), p.root.as_path())).collect();
        assert_eq!(
            named,
            [("beta", hosted.path()), ("acme", plugins.join("acme-tools").as_path())],
            "the host's first; a folder's own copy of a name the host has is left out"
        );
    }
}
