//! The layout of a Claude Code home directory (`~/.claude`), as found on
//! real installs:
//!
//! - `skills/<name>/SKILL.md` and `agents/<name>.md`: the user's own.
//! - `skills/synced/<bucket>/<name>/SKILL.md`: skills synced from
//!   claude.ai; Claude Code exposes them as `anthropic-skills:<name>`.
//! - `plugins/installed_plugins.json`: installed plugins and their
//!   `installPath`, which holds `skills/` and `agents/`; their items are
//!   namespaced `<plugin>:<name>`. `plugins/marketplaces/` only holds
//!   catalogues of plugins that are *available*, so it is not scanned.
//! - `settings.json` → `enabledPlugins`: a plugin set to `false` is off.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub const SYNCED_NAMESPACE: &str = "anthropic-skills";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub dir: PathBuf,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPlugin {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeHome {
    root: PathBuf,
}

impl ClaudeHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn detect() -> Option<Self> {
        std::env::home_dir().filter(|home| !home.as_os_str().is_empty()).map(|home| Self::new(home.join(".claude")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn skill_sources(&self, repo: &Path) -> Vec<Source> {
        let mut sources = vec![Source { dir: self.root.join("skills"), namespace: None }];
        sources.extend(
            subdirs(&self.root.join("skills").join("synced"))
                .into_iter()
                .map(|dir| Source { dir, namespace: Some(SYNCED_NAMESPACE.to_owned()) }),
        );
        sources.extend(self.plugin_sources(repo, "skills"));
        sources
    }

    pub fn agent_sources(&self, repo: &Path) -> Vec<Source> {
        let mut sources = vec![Source { dir: self.root.join("agents"), namespace: None }];
        sources.extend(self.plugin_sources(repo, "agents"));
        sources
    }

    fn plugin_sources(&self, repo: &Path, kind: &str) -> Vec<Source> {
        self.installed_plugins(repo)
            .into_iter()
            .map(|plugin| Source { dir: plugin.path.join(kind), namespace: Some(plugin.name) })
            .collect()
    }

    pub fn installed_plugins(&self, repo: &Path) -> Vec<InstalledPlugin> {
        let installed = read_json(&self.root.join("plugins").join("installed_plugins.json"));
        let settings = read_json(&self.root.join("settings.json"));
        parse_installed(&installed, &settings, repo)
    }
}

pub fn parse_installed(installed: &Value, settings: &Value, repo: &Path) -> Vec<InstalledPlugin> {
    let Some(plugins) = installed.get("plugins").and_then(Value::as_object) else { return Vec::new() };
    let mut found: Vec<InstalledPlugin> = Vec::new();
    for (id, entries) in plugins {
        if settings["enabledPlugins"].get(id).and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let name = id.split('@').next().unwrap_or(id).to_owned();
        let entries: Vec<&Value> = match entries {
            Value::Array(list) => list.iter().collect(),
            single => vec![single],
        };
        let in_scope = |entry: &&Value| match entry.get("projectPath").and_then(Value::as_str) {
            Some(project) => Path::new(project) == repo,
            None => true,
        };
        if let Some(path) = entries.into_iter().filter(in_scope).find_map(|entry| entry.get("installPath")?.as_str()) {
            found.push(InstalledPlugin { name, path: PathBuf::from(path) });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Value::Null)
}

pub(crate) fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
    dirs.sort();
    dirs
}

pub(crate) fn qualified(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(namespace) => format!("{namespace}:{name}"),
        None => name.to_owned(),
    }
}

pub fn bare_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_v2_installs_with_scope_and_enablement() {
        let installed = json!({"version": 2, "plugins": {
            "review@official": [{"scope": "user", "installPath": "/c/review/1.0"}],
            "off@official": [{"scope": "user", "installPath": "/c/off/1.0"}],
            "local@official": [{"scope": "project", "projectPath": "/other", "installPath": "/c/local/1.0"}],
            "mine@official": [{"scope": "project", "projectPath": "/repo", "installPath": "/c/mine/1.0"}]
        }});
        let settings = json!({"enabledPlugins": {"off@official": false, "review@official": true}});
        let plugins = parse_installed(&installed, &settings, Path::new("/repo"));
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["mine", "review"]);
        assert_eq!(plugins[1].path, PathBuf::from("/c/review/1.0"));
    }

    #[test]
    fn parses_v1_installs_and_tolerates_garbage() {
        let installed = json!({"version": 1, "plugins": {"a@m": {"installPath": "/c/a"}}});
        assert_eq!(parse_installed(&installed, &Value::Null, Path::new("/r"))[0].name, "a");
        assert!(parse_installed(&json!([1, 2]), &Value::Null, Path::new("/r")).is_empty());
    }

    #[test]
    fn names_are_qualified_and_bared() {
        assert_eq!(qualified(Some("anthropic-skills"), "pdf"), "anthropic-skills:pdf");
        assert_eq!(qualified(None, "pdf"), "pdf");
        assert_eq!(bare_name("anthropic-skills:pdf"), "pdf");
        assert_eq!(bare_name("pdf"), "pdf");
    }
}
