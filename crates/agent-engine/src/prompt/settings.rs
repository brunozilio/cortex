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
    #[serde(default)]
    pub judge: Option<String>,
    #[serde(default)]
    pub fallbacks: Vec<String>,
    #[serde(default)]
    pub small_model: Option<String>,
    #[serde(default)]
    pub enabled_providers: Vec<String>,
    #[serde(default)]
    pub mcp_approval: domain::mcp_config::McpApproval,
    #[serde(default)]
    pub lsp_servers: Vec<LspLayer>,
    #[serde(skip)]
    pub permissions: domain::permission_rules::PermissionRules,
    #[serde(default)]
    pub added_dirs: Vec<String>,
    pub scripts: crate::tools::script_tool::ScriptAccess,
}

pub type LspLayer = std::collections::BTreeMap<String, domain::lsp::ServerConfig>;
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
            fallbacks: Vec::new(),
            small_model: None,
            enabled_providers: Vec::new(),
            mcp_approval: Default::default(),
            lsp_servers: Vec::new(),
            permissions: Default::default(),
            added_dirs: Vec::new(),
            scripts: Default::default(),
        }
    }
}
fn enabled() -> bool {
    true
}
fn model_name(value: &Value, key: &str) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let name = value.as_str().filter(|s| !s.is_empty() && !s.contains(char::is_whitespace));
    Ok(Some(name.ok_or(format!("{key} must be a model name"))?.to_owned()))
}
fn model_names(value: &Value, key: &str) -> Result<Vec<String>, String> {
    match value.as_array() {
        Some(models) => models.iter().map(|model| model_name(model, key)).filter_map(Result::transpose).collect(),
        None => Ok(model_name(value, key)?.into_iter().collect()),
    }
}
pub fn load(cwd: &Path, home: Option<&Path>, plugins: &[crate::plugins::Plugin]) -> Result<Settings, String> {
    let mut selected = String::from("Default");
    let mut schema = None;
    let mut agent = None;
    let mut auto_memory = true;
    let mut advisor = None;
    let mut judge = None;
    let mut fallbacks = Vec::new();
    let mut small_model = None;
    let mut enabled_providers = Vec::new();
    let mut hooks = crate::hooks::Hooks::default();
    let mut mcp_approval = domain::mcp_config::McpApproval::default();
    let mut lsp_servers = Vec::new();
    let mut permissions = domain::permission_rules::PermissionRules::default();
    let mut added_dirs = Vec::new();
    let mut scripts = crate::tools::script_tool::ScriptAccess::default();
    let is_user_root = |root: &Path| home.is_some_and(|home| root == home.join(brand::DIR));
    let mut roots = Vec::new();
    if let Some(home) = home {
        roots.push(home.join(brand::DIR));
    }
    let mut ancestors: Vec<_> = cwd.ancestors().collect();
    ancestors.reverse();
    for directory in ancestors {
        roots.push(directory.join(brand::DIR));
    }
    let mut seen = std::collections::HashSet::new();
    roots.retain(|root| seen.insert(root.canonicalize().unwrap_or_else(|_| root.clone())));
    for root in &roots {
        hooks.extend(&crate::hooks::Hooks::from_scripts(&root.join("hooks")));
    }
    for root in &roots {
        for name in ["settings.json", "settings.local.json"] {
            let path = root.join(name);
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            };
            let settings: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let owners = is_user_root(root) || name == "settings.local.json";
            if owners {
                mcp_approval.read(&settings);
                let named = settings["enabledProviders"].as_array().into_iter().flatten();
                enabled_providers.extend(named.filter_map(|id| id.as_str().map(str::to_owned)));
            }
            super::permissions::read(&settings, owners, &mut permissions, &mut added_dirs)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            if let Some(value) = settings.get("scripts") {
                scripts = crate::tools::script_tool::ScriptAccess::read(value)?;
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
            if let Some(value) = settings.get("fallbackModel") {
                fallbacks = model_names(value, "fallbackModel")?;
            }
            if let Some(value) = settings.get("smallModel") {
                small_model = model_name(value, "smallModel")?;
            }
            if let Some(value) = settings.get("hooks") {
                hooks
                    .extend(&crate::hooks::Hooks::parse(value).map_err(|e| format!("{}: hooks: {e}", path.display()))?);
            }
            if let Some(value) = settings.get("lspServers") {
                let layer: LspLayer = serde_json::from_value(value.clone())
                    .map_err(|e| format!("{}: lspServers: {e}", path.display()))?;
                lsp_servers.push(if owners {
                    layer
                } else {
                    layer.into_iter().map(|(server, config)| (server, config.without_commands())).collect()
                });
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
    lsp_servers.extend(crate::plugins::lsp_layers(plugins));
    if ["CLAUDE_CODE_DISABLE_AUTO_MEMORY", concat!(brand::env_prefix!(), "_DISABLE_AUTO_MEMORY")]
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
                fallbacks,
                small_model,
                enabled_providers,
                mcp_approval,
                lsp_servers,
                permissions,
                added_dirs,
                scripts,
            });
        }
    };
    Ok(Settings {
        style,
        schema,
        agent,
        hooks,
        auto_memory,
        advisor,
        judge,
        fallbacks,
        small_model,
        enabled_providers,
        mcp_approval,
        lsp_servers,
        permissions,
        added_dirs,
        scripts,
    })
}
#[cfg(test)]
#[path = "settings_lsp_tests.rs"]
mod lsp_tests;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
