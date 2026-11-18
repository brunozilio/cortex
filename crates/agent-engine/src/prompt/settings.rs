use crate::frontmatter::parse as parse_frontmatter;
use crate::trust::Gate;
use serde_json::Value;
use std::path::{Path, PathBuf};
pub const MANAGED: &str = "managed-settings.json";
const LOCAL: &str = "settings.local.json";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Style {
    pub name: String,
    pub instructions: String,
    pub keep_coding: bool,
    #[serde(default)]
    pub reminder: Option<String>,
}
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub style: Option<Style>,
    pub schema: Option<Value>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub hooks: crate::hooks::Hooks,
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
    pub teammate_mode: domain::team::TeammateMode,
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
    #[serde(default)]
    pub skipped_files: Vec<PathBuf>,
    #[serde(default)]
    pub bash_output: domain::output::OutputLimit,
}

pub type LspLayer = std::collections::BTreeMap<String, domain::lsp::ServerConfig>;
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

pub fn load(gate: &Gate, plugins: &[crate::plugins::Plugin]) -> Result<Settings, String> {
    let home = gate.home();
    let roots = roots(gate.cwd(), home);
    let is_user_root = |root: &Path| home.is_some_and(|home| root == home.join(brand::DIR));
    let mut merged = Settings::default();
    merged.hooks.extend(&crate::hooks::Hooks::gathered(home, &roots, &|file| gate.honours(file)));
    merged.project_criteria = super::criteria_file::nearest(roots.iter().filter(|root| !is_user_root(root)));
    let mut style = String::from("Default");
    let mut replaced_opt_in = false;
    let mut problems = Vec::new();
    let mut skipped = Vec::new();
    for (path, users) in files(&roots, home) {
        let honoured = users || gate.honours(&path);
        let owners = users || (honoured && path.file_name().is_some_and(|name| name == LOCAL));
        let value = match read(&path) {
            Ok(Some(value)) => value,
            Ok(None) => continue,
            Err(error) if owners => return Err(error),
            Err(error) => {
                skipped.push((path, error));
                continue;
            }
        };
        let checked = crate::config_schema::checked_settings(value);
        problems.extend(checked.located(&path));
        let value = checked.value;
        replaced_opt_in |= users && value.get(domain::trust::LEGACY_OPT_IN).is_some();
        let layer = Layer { path: &path, value: &value, owners, runs_hooks: honoured };
        if owners {
            layer.merge_into(&mut merged, &mut style)?;
            continue;
        }
        let (mut tried, mut tried_style) = (merged.clone(), style.clone());
        match layer.merge_into(&mut tried, &mut tried_style) {
            Ok(()) => (merged, style) = (tried, tried_style),
            Err(error) => skipped.push((path, error)),
        }
    }
    add_plugins(&mut merged, plugins);
    problems.extend(skipped.iter().map(|(path, error)| skipped_layer(path, error)));
    merged.skipped_files = skipped.into_iter().map(|(path, _)| path).collect();
    merged.notices = gate.skipped_notice().into_iter().collect();
    merged.notices.extend(replaced_opt_in.then(domain::trust::legacy_notice));
    crate::config_schema::logged_once(&problems);
    merged.notices.extend(problems);
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
        .flat_map(|root| ["settings.json", LOCAL].map(|name| (root.join(name), is_user_root(root))))
        .collect();
    files.extend(home.map(|home| (home.join(brand::DIR).join(MANAGED), true)));
    files
}

fn skipped_layer(path: &Path, error: &str) -> String {
    let shown = path.display().to_string();
    let why = error.strip_prefix(&shown).and_then(|rest| rest.strip_prefix(": ")).unwrap_or(error);
    format!("Skipped {shown}: {why}; the rest of the settings apply. Fix or remove that file.")
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
    runs_hooks: bool,
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
        if let Some(value) = self.value.get("bashOutputLimit") {
            let characters = value.as_u64().ok_or("bashOutputLimit must be a whole number of characters")?;
            merged.bash_output = domain::output::OutputLimit::of(characters);
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
        if let Some(value) = settings.get("teammateMode") {
            merged.teammate_mode = value
                .as_str()
                .and_then(domain::team::TeammateMode::named)
                .ok_or("teammateMode must be process, tmux, iterm2 or auto")?;
        }
        Ok(())
    }

    fn merge_extensions(&self, merged: &mut Settings) -> Result<(), String> {
        if let Some(value) = self.value.get("hooks").filter(|_| self.runs_hooks) {
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
pub(crate) fn load_trusted(
    cwd: &Path,
    home: Option<&Path>,
    plugins: &[crate::plugins::Plugin],
) -> Result<Settings, String> {
    load(&Gate::at(cwd, home, domain::trust::Trust::Trusted), plugins)
}
#[cfg(test)]
#[path = "settings_lsp_tests.rs"]
mod lsp_tests;
#[cfg(test)]
#[path = "settings_schema_tests.rs"]
mod schema_tests;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "settings_trust_tests.rs"]
mod trust_tests;
