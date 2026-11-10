//! User/project output settings; native settings override compatibility files.
use crate::frontmatter::parse as parse_frontmatter;
use serde_json::Value;
use std::path::Path;
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Style {
    pub name: String,
    pub instructions: String,
    pub keep_coding: bool,
    #[serde(default)]
    pub reminder: Option<String>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub style: Option<Style>,
    pub schema: Option<Value>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub hooks: crate::hooks::Hooks,
    #[serde(default = "enabled")]
    pub auto_memory: bool,
    #[serde(default)]
    pub advisor: Option<String>,
    /// The model that judges verification (`judgeModel`); the run's own
    /// when unset.
    #[serde(default)]
    pub judge: Option<String>,
    /// The owner's say on the project's `.mcp.json` servers: read only from
    /// the user's settings and the project's `settings.local.json`.
    #[serde(default)]
    pub mcp_approval: superfik_core::mcp_config::McpApproval,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            style: None,
            schema: None,
            agent: None,
            hooks: Default::default(),
            auto_memory: true,
            advisor: None,
            judge: None,
            mcp_approval: Default::default(),
        }
    }
}
fn enabled() -> bool {
    true
}
/// A model setting: a name, or `null` to unset one from a broader file.
fn model_name(value: &Value, key: &str) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let name = value.as_str().filter(|s| !s.is_empty() && !s.contains(char::is_whitespace));
    Ok(Some(name.ok_or(format!("{key} must be a model name"))?.to_owned()))
}
pub fn load(cwd: &Path, home: Option<&Path>, plugins: &[crate::plugins::Plugin]) -> Result<Settings, String> {
    let mut selected = String::from("Default");
    let mut schema = None;
    let mut agent = None;
    let mut auto_memory = true;
    let mut advisor = None;
    let mut judge = None;
    let mut hooks = crate::hooks::Hooks::default();
    let mut mcp_approval = superfik_core::mcp_config::McpApproval::default();
    let is_user_root =
        |root: &Path| home.is_some_and(|home| root == home.join(".claude") || root == home.join(".superfik"));
    let mut roots = Vec::new();
    if let Some(home) = home {
        roots.extend([home.join(".claude"), home.join(".superfik")]);
    }
    let mut ancestors: Vec<_> = cwd.ancestors().collect();
    ancestors.reverse();
    for directory in ancestors {
        roots.extend([directory.join(".claude"), directory.join(".superfik")]);
    }
    let mut seen = std::collections::HashSet::new();
    roots.retain(|root| seen.insert(root.canonicalize().unwrap_or_else(|_| root.clone())));
    for root in &roots {
        for name in ["settings.json", "settings.local.json"] {
            let path = root.join(name);
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            };
            let settings: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            if is_user_root(root) || name == "settings.local.json" {
                mcp_approval.read(&settings);
            }
            if let Some(value) = settings.get("autoMemoryEnabled") {
                auto_memory = value.as_bool().ok_or("autoMemoryEnabled must be boolean")?;
            }
            if let Some(value) = settings.get("agent") {
                agent = if value.is_null() {
                    None
                } else {
                    Some(value.as_str().ok_or("agent setting must be a name")?.to_owned())
                };
            }
            if let Some(value) = settings.get("advisorModel") {
                advisor = model_name(value, "advisorModel")?;
            }
            if let Some(value) = settings.get("judgeModel") {
                judge = model_name(value, "judgeModel")?;
            }
            if let Some(value) = settings.get("hooks") {
                hooks.extend(&crate::hooks::Hooks::parse(value)?);
            }
            if let Some(style) = settings["outputStyle"].as_str() {
                selected = style.to_owned();
            }
            if let Some(value) = settings.get("jsonSchema") {
                schema = (!value.is_null()).then(|| value.clone());
            }
        }
    }
    for plugin in plugins {
        match plugin.hooks().and_then(|events| events.map(|events| crate::hooks::Hooks::parse(&events)).transpose()) {
            Ok(Some(found)) => hooks.extend(&found),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, plugin = %plugin.name, "a plugin's hooks are left out"),
        }
    }
    if ["CLAUDE_CODE_DISABLE_AUTO_MEMORY", "SUPERFIK_DISABLE_AUTO_MEMORY"]
        .iter()
        .any(|key| std::env::var(key).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes")))
    {
        auto_memory = false;
    }
    let style = match selected.to_ascii_lowercase().as_str() {
        "default" => None,
        name if super::styles::builtin(name).is_some() => super::styles::builtin(name).map(|style| style.style()),
        _ => {
            if selected.is_empty() || !selected.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)) {
                return Err("invalid outputStyle name".into());
            }
            let path = roots
                .iter()
                .rev()
                .map(|r| r.join("output-styles").join(format!("{selected}.md")))
                .find(|p| p.is_file())
                .ok_or_else(|| format!("output style {selected} was not found"))?;
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let (front, body) = parse_frontmatter(&text);
            return Ok(Settings {
                style: Some(Style {
                    name: selected,
                    instructions: body.trim().into(),
                    keep_coding: front.get("keep-coding-instructions").is_some_and(|v| v == "true"),
                    reminder: front.get("turn-reminder").map(|text| text.trim().to_owned()).filter(|t| !t.is_empty()),
                }),
                schema,
                agent,
                hooks,
                auto_memory,
                advisor,
                judge,
                mcp_approval,
            });
        }
    };
    Ok(Settings { style, schema, agent, hooks, auto_memory, advisor, judge, mcp_approval })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_overrides_user_and_native_overrides_compatibility_style() {
        let home = tempfile::tempdir().expect("valid test fixture");
        let repo = tempfile::tempdir().expect("valid test fixture");
        for (root, style) in [
            (home.path().join(".superfik"), "Explanatory"),
            (repo.path().join(".claude"), "Learning"),
            (repo.path().join(".superfik"), "Concise"),
        ] {
            std::fs::create_dir_all(&root).expect("valid test fixture");
            std::fs::write(root.join("settings.json"), serde_json::json!({"outputStyle":style}).to_string())
                .expect("valid test fixture");
        }
        let config = load(repo.path(), Some(home.path()), &[]).expect("valid test fixture");
        assert_eq!(config.style.expect("valid test fixture").name, "Concise");
    }
    #[test]
    fn custom_style_controls_coding_instructions_and_rejects_paths() {
        let repo = tempfile::tempdir().expect("valid test fixture");
        let root = repo.path().join(".superfik");
        std::fs::create_dir_all(root.join("output-styles")).expect("valid test fixture");
        std::fs::write(root.join("settings.json"), r#"{"outputStyle":"review"}"#).expect("valid test fixture");
        std::fs::write(
            root.join("output-styles/review.md"),
            "---\nkeep-coding-instructions: false\n---\nReturn review findings.",
        )
        .expect("valid test fixture");
        let style = load(repo.path(), None, &[]).expect("valid test fixture").style.expect("valid test fixture");
        assert!(!style.keep_coding);
        assert_eq!(style.instructions, "Return review findings.");
        std::fs::write(root.join("settings.json"), r#"{"outputStyle":"../private"}"#).expect("valid test fixture");
        assert!(load(repo.path(), None, &[]).is_err());
    }
    #[test]
    fn judge_model_names_the_verification_model() {
        let repo = tempfile::tempdir().expect("repo");
        let root = repo.path().join(".superfik");
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("settings.json"), r#"{"judgeModel":"claude-haiku-5-5"}"#).expect("write");
        assert_eq!(load(repo.path(), None, &[]).expect("settings").judge.as_deref(), Some("claude-haiku-5-5"));
        std::fs::write(root.join("settings.json"), r#"{"judgeModel":"two words"}"#).expect("write");
        assert!(load(repo.path(), None, &[]).is_err());
    }
    #[test]
    fn only_the_owner_approves_a_project_s_mcp_servers() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let write = |path: std::path::PathBuf, text: &str| {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            std::fs::write(path, text).expect("write");
        };
        write(repo.path().join(".superfik/settings.json"), r#"{"enableAllProjectMcpServers": true}"#);
        write(repo.path().join(".claude/settings.local.json"), r#"{"enabledMcpjsonServers": ["jira"]}"#);
        write(home.path().join(".superfik/settings.json"), r#"{"disabledMcpjsonServers": ["sonar"]}"#);
        let approval = load(repo.path(), Some(home.path()), &[]).expect("settings").mcp_approval;
        assert!(!approval.all, "a checked-in setting cannot approve its own servers");
        assert_eq!(approval.enabled, ["jira"]);
        assert_eq!(approval.disabled, ["sonar"]);
    }
    #[test]
    fn an_installed_plugin_adds_its_hooks_and_a_broken_one_is_left_out() {
        let repo = tempfile::tempdir().expect("repo");
        let good = tempfile::tempdir().expect("plugin");
        let broken = tempfile::tempdir().expect("plugin");
        std::fs::create_dir_all(good.path().join("hooks")).expect("hooks dir");
        std::fs::write(
            good.path().join("hooks/hooks.json"),
            r#"{"description": "Checks", "hooks": {"PreToolUse": [{"matcher": "Bash",
                "hooks": [{"type": "command", "command": "${SUPERFIK_PLUGIN_ROOT}/check.sh"}]}]}}"#,
        )
        .expect("hooks");
        std::fs::create_dir_all(broken.path().join("hooks")).expect("hooks dir");
        std::fs::write(broken.path().join("hooks/hooks.json"), r#"{"hooks": {"NoSuchEvent": []}}"#).expect("hooks");
        let plugins = [crate::plugins::Plugin::at(good.path()), crate::plugins::Plugin::at(broken.path())];
        let settings = load(repo.path(), None, &plugins).expect("a broken plugin does not stop the run");
        let groups = &settings.hooks.0["PreToolUse"];
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].matcher, "Bash");
        assert_eq!(groups[0].hooks[0].command, format!("{}/check.sh", good.path().display()));
        assert!(!settings.hooks.0.contains_key("NoSuchEvent"));
    }
}
