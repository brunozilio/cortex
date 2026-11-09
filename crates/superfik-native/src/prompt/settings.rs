//! User/project output settings; native settings override compatibility files.
use serde_json::Value;
use std::path::Path;
use superfik_plugins::skills::parse_frontmatter;
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
}
impl Default for Settings {
    fn default() -> Self {
        Self { style: None, schema: None, agent: None, hooks: Default::default(), auto_memory: true, advisor: None }
    }
}
fn enabled() -> bool {
    true
}
pub fn load(cwd: &Path, home: Option<&Path>) -> Result<Settings, String> {
    let mut selected = String::from("Default");
    let mut schema = None;
    let mut agent = None;
    let mut auto_memory = true;
    let mut advisor = None;
    let mut hooks = crate::hooks::Hooks::default();
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
                advisor = if value.is_null() {
                    None
                } else {
                    Some(
                        value
                            .as_str()
                            .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace))
                            .ok_or("advisorModel must be a model name")?
                            .to_owned(),
                    )
                };
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
            });
        }
    };
    Ok(Settings { style, schema, agent, hooks, auto_memory, advisor })
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
        let config = load(repo.path(), Some(home.path())).expect("valid test fixture");
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
        let style = load(repo.path(), None).expect("valid test fixture").style.expect("valid test fixture");
        assert!(!style.keep_coding);
        assert_eq!(style.instructions, "Return review findings.");
        std::fs::write(root.join("settings.json"), r#"{"outputStyle":"../private"}"#).expect("valid test fixture");
        assert!(load(repo.path(), None).is_err());
    }
}
