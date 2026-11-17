use super::project_trust::ProjectTrust;
use crate::frontmatter::parse as parse_frontmatter;
use serde_json::Value;
use std::path::{Path, PathBuf};
pub const MANAGED: &str = "managed-settings.json";

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
    pub escalation: Option<String>,
    #[serde(default)]
    pub project_criteria: Option<String>,
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
    #[serde(skip)]
    pub judge_allow: domain::permission_rules::PermissionRules,
    #[serde(default)]
    pub added_dirs: Vec<String>,
    pub scripts: crate::tools::script_tool::ScriptGrants,
    #[serde(default)]
    pub features: domain::features::Features,
    #[serde(default)]
    pub sandbox: domain::sandbox::Sandbox,
    #[serde(default)]
    pub notices: Vec<String>,
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
            escalation: None,
            project_criteria: None,
            fallbacks: Vec::new(),
            small_model: None,
            enabled_providers: Vec::new(),
            mcp_approval: Default::default(),
            lsp_servers: Vec::new(),
            permissions: Default::default(),
            judge_allow: Default::default(),
            added_dirs: Vec::new(),
            scripts: Default::default(),
            features: Default::default(),
            sandbox: Default::default(),
            notices: Vec::new(),
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
    let roots = roots(cwd, home);
    let trust = ProjectTrust::of(home);
    let is_user_root = |root: &Path| home.is_some_and(|home| root == home.join(brand::DIR));
    let mut merged = Settings::default();
    merged.hooks.extend(&crate::hooks::Hooks::gathered(home, &roots, &|file| trust.runs_hooks_of(file)));
    merged.project_criteria = super::criteria_file::nearest(roots.iter().filter(|root| !is_user_root(root)));
    let mut style = String::from("Default");
    for (path, owners) in files(&roots, home) {
        let Some(value) = read(&path)? else { continue };
        let owners = owners && trust.local_is_the_owner_s(&path);
        let layer = Layer { path: &path, value: &value, owners, trust: &trust };
        layer.merge_into(&mut merged, &mut style)?;
    }
    add_plugins(&mut merged, plugins);
    merged.notices = trust.notices();
    if auto_memory_disabled_by_environment() {
        merged.auto_memory = false;
    }
    merged.style = style_named(style, &roots)?;
    Ok(merged)
}

fn roots(cwd: &Path, home: Option<&Path>) -> Vec<PathBuf> {
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
    roots
}

fn files(roots: &[PathBuf], home: Option<&Path>) -> Vec<(PathBuf, bool)> {
    let is_user_root = |root: &Path| home.is_some_and(|home| root == home.join(brand::DIR));
    let mut files: Vec<(PathBuf, bool)> = roots
        .iter()
        .flat_map(|root| {
            ["settings.json", "settings.local.json"]
                .map(|name| (root.join(name), is_user_root(root) || name == "settings.local.json"))
        })
        .collect();
    files.extend(home.map(|home| (home.join(brand::DIR).join(MANAGED), true)));
    files
}

fn read(path: &Path) -> Result<Option<Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    serde_json::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display()))
}

struct Layer<'a> {
    path: &'a Path,
    value: &'a Value,
    owners: bool,
    trust: &'a ProjectTrust,
}

impl Layer<'_> {
    fn merge_into(&self, merged: &mut Settings, style: &mut String) -> Result<(), String> {
        self.merge_access(merged)?;
        self.merge_agent(merged)?;
        self.merge_models(merged)?;
        self.merge_extensions(merged)?;
        if let Some(name) = self.value["outputStyle"].as_str() {
            name.clone_into(style);
        }
        if let Some(value) = self.value.get("jsonSchema") {
            merged.schema = (!value.is_null()).then(|| value.clone());
        }
        Ok(())
    }

    fn located(&self, error: impl std::fmt::Display) -> String {
        format!("{}: {error}", self.path.display())
    }

    fn merge_access(&self, merged: &mut Settings) -> Result<(), String> {
        let settings = self.value;
        if self.owners {
            merged.mcp_approval.read(settings);
            let named = settings["enabledProviders"].as_array().into_iter().flatten();
            merged.enabled_providers.extend(named.filter_map(|id| id.as_str().map(str::to_owned)));
        }
        super::permissions::read(settings, self.owners, &mut merged.permissions, &mut merged.added_dirs)
            .map_err(|e| self.located(e))?;
        if self.owners {
            super::permissions::read_judge(settings, &mut merged.judge_allow).map_err(|e| self.located(e))?;
        }
        if let Some(value) = settings.get("scripts") {
            merged.scripts = merged.scripts.layered(value, self.owners)?;
        }
        if let Some(value) = settings.get("features") {
            let features = std::mem::take(&mut merged.features);
            merged.features = features.layered(value, self.owners).map_err(|e| self.located(e))?;
        }
        if let Some(value) = settings.get("sandbox") {
            let sandbox = std::mem::take(&mut merged.sandbox);
            merged.sandbox = sandbox.layered(value, self.owners).map_err(|e| self.located(e))?;
        }
        Ok(())
    }

    fn merge_agent(&self, merged: &mut Settings) -> Result<(), String> {
        if let Some(value) = self.value.get("autoMemoryEnabled") {
            merged.auto_memory = value.as_bool().ok_or("autoMemoryEnabled must be boolean")?;
        }
        if let Some(value) = self.value.get("agent") {
            merged.agent = if value.is_null() {
                None
            } else {
                Some(value.as_str().ok_or("agent setting must be a name")?.to_owned())
            };
        }
        Ok(())
    }

    fn merge_models(&self, merged: &mut Settings) -> Result<(), String> {
        let settings = self.value;
        if let Some(value) = settings.get("advisorModel") {
            merged.advisor = model_name(value, "advisorModel")?;
        }
        if let Some(value) = settings.get("judgeModel") {
            merged.judge = model_name(value, "judgeModel")?;
        }
        if self.owners
            && let Some(value) = settings.get("escalationModel")
        {
            merged.escalation = model_name(value, "escalationModel")?;
        }
        if let Some(value) = settings.get("fallbackModel") {
            merged.fallbacks = model_names(value, "fallbackModel")?;
        }
        if let Some(value) = settings.get("smallModel") {
            merged.small_model = model_name(value, "smallModel")?;
        }
        Ok(())
    }

    fn merge_extensions(&self, merged: &mut Settings) -> Result<(), String> {
        if let Some(value) = self.value.get("hooks").filter(|_| self.trust.runs_hooks_of(self.path)) {
            let hooks = crate::hooks::Hooks::parse(value).map_err(|e| self.located(format!("hooks: {e}")))?;
            merged.hooks.extend(&hooks);
        }
        if let Some(value) = self.value.get("lspServers") {
            let layer: LspLayer =
                serde_json::from_value(value.clone()).map_err(|e| self.located(format!("lspServers: {e}")))?;
            merged.lsp_servers.push(if self.owners {
                layer
            } else {
                layer.into_iter().map(|(server, config)| (server, config.without_commands())).collect()
            });
        }
        Ok(())
    }
}

fn add_plugins(merged: &mut Settings, plugins: &[crate::plugins::Plugin]) {
    for plugin in plugins {
        match plugin.hooks().and_then(|events| events.map(|events| crate::hooks::Hooks::parse(&events)).transpose()) {
            Ok(Some(found)) => merged.hooks.extend(&found),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, plugin = %plugin.name, "a plugin's hooks are left out"),
        }
    }
    merged.lsp_servers.extend(crate::plugins::lsp_layers(plugins));
}

fn auto_memory_disabled_by_environment() -> bool {
    ["CLAUDE_CODE_DISABLE_AUTO_MEMORY", concat!(brand::env_prefix!(), "_DISABLE_AUTO_MEMORY")]
        .iter()
        .any(|key| std::env::var(key).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes")))
}

fn style_named(selected: String, roots: &[PathBuf]) -> Result<Option<Style>, String> {
    match selected.to_ascii_lowercase().as_str() {
        "default" => return Ok(None),
        name if super::styles::builtin(name).is_some() => {
            return Ok(super::styles::builtin(name).map(|style| style.style()));
        }
        _ => {}
    }
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
    Ok(Some(Style {
        name: selected,
        instructions: body.trim().into(),
        keep_coding: front.get("keep-coding-instructions").is_some_and(|v| v == "true"),
        reminder: front.get("turn-reminder").map(|text| text.trim().to_owned()).filter(|t| !t.is_empty()),
    }))
}
#[cfg(test)]
#[path = "settings_lsp_tests.rs"]
mod lsp_tests;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
