use domain::{Effort, ModelProvider, PermissionMode};
use serde_json::Value;
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    pub tools: Option<Vec<String>>,
    pub disallowed_tools: Vec<String>,
    pub mcp_servers: Option<Vec<String>>,
    pub permission: Option<PermissionMode>,
    pub max_turns: Option<usize>,
    pub effort: Option<Effort>,
    pub model: Option<String>,
    pub skills: Vec<String>,
    pub background: bool,
    #[serde(default)]
    pub dont_ask: bool,
    #[serde(default)]
    pub omit_instructions: bool,
    #[serde(default)]
    pub initial_prompt: Option<String>,
    #[serde(default)]
    pub compact_window: Option<u64>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub worktree: bool,
    #[serde(default)]
    pub hooks: crate::hooks::Hooks,
    #[serde(default)]
    pub observers: Vec<String>,
    #[serde(default)]
    pub observer_message: Option<String>,
    #[serde(default)]
    pub unobserved_subagents: bool,
    pub error: Option<String>,
}
impl AgentConfig {
    pub fn parse(value: &Value) -> Result<Self, String> {
        let list = |key: &str| crate::prompt::markdown::list(&value[key]).map_err(|e| format!("{key}: {e}"));
        let mut config = Self {
            tools: list("tools")?,
            disallowed_tools: list("disallowedTools")?.unwrap_or_default(),
            mcp_servers: list("mcpServers")?,
            skills: list("skills")?.unwrap_or_default(),
            ..Default::default()
        };
        if let Some(value) = value.get("omitClaudeMd") {
            config.omit_instructions = value.as_bool().ok_or("omitClaudeMd must be boolean")?;
        }
        if let Some(prompt) = value.get("initialPrompt") {
            config.initial_prompt = Some(prompt.as_str().ok_or("initialPrompt must be a string")?.into());
        }
        if let Some(window) = value.get("autoCompactWindow") {
            config.compact_window = Some(
                window
                    .as_u64()
                    .filter(|n| (100_000..=1_000_000).contains(n))
                    .ok_or("autoCompactWindow must be within 100000..1000000 tokens")?,
            );
        }
        if let Some(scope) = value.get("memory") {
            config.memory = Some(
                scope
                    .as_str()
                    .filter(|s| matches!(*s, "user" | "project" | "local"))
                    .ok_or("memory must be user, project or local")?
                    .into(),
            );
        }
        if let Some(isolation) = value.get("isolation") {
            if isolation.as_str() != Some("worktree") {
                return Err("isolation must be worktree".into());
            }
            config.worktree = true;
        }
        if let Some(hooks) = value.get("hooks") {
            config.hooks = crate::hooks::Hooks::parse(hooks)?;
        }
        for name in config.tools.iter().flatten().chain(&config.disallowed_tools) {
            if name.is_empty()
                || !name.trim_end_matches('*').chars().all(|c| c.is_ascii_alphanumeric() || "_-:".contains(c))
            {
                return Err("tool selectors must be names or trailing-prefix wildcards".into());
            }
        }
        if let Some(mode) = value.get("permissionMode") {
            config.permission = Some(match mode.as_str() {
                Some(name) if name.eq_ignore_ascii_case("dontAsk") => {
                    config.dont_ask = true;
                    PermissionMode::Ask
                }
                Some(name) => domain::command::mode_named(name).ok_or("unsupported custom-agent permissionMode")?,
                None => return Err("unsupported custom-agent permissionMode".into()),
            });
        }
        if let Some(turns) = value.get("maxTurns") {
            config.max_turns = Some(
                turns
                    .as_u64()
                    .filter(|n| *n > 0 && *n <= crate::agent::MAX_TURNS as u64)
                    .ok_or("maxTurns must be within 1..200")? as usize,
            );
        }
        if let Some(effort) = value.get("effort") {
            config.effort = Some(match effort.as_str() {
                Some("low") => Effort::Low,
                Some("medium") => Effort::Medium,
                Some("high") => Effort::High,
                Some("xhigh") => Effort::XHigh,
                Some("max") => Effort::Max,
                _ => return Err("invalid custom-agent effort".into()),
            });
        }
        if let Some(model) = value.get("model") {
            let model = model
                .as_str()
                .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace))
                .ok_or("invalid custom-agent model")?;
            if model != "inherit" {
                config.model = Some(model.into());
            }
        }
        if let Some(background) = value.get("background") {
            config.background = background.as_bool().ok_or("background must be boolean")?;
        }
        config.observe(value)?;
        Ok(config)
    }

    fn observe(&mut self, value: &Value) -> Result<(), String> {
        if let Some(named) = crate::prompt::markdown::list(&value["observer"]).map_err(|e| format!("observer: {e}"))? {
            let (kept, dropped) = domain::observer::observers(&named.join(","));
            if !dropped.is_empty() {
                tracing::warn!(
                    ?dropped,
                    "an agent has at most {} observers; these are left out",
                    domain::observer::MOST
                );
            }
            self.observers = kept;
        }
        if let Some(message) = value.get("observerMessage") {
            self.observer_message = Some(message.as_str().ok_or("observerMessage must be a string")?.to_owned());
        }
        if let Some(subagents) = value.get("observeSubagents") {
            self.unobserved_subagents = !subagents.as_bool().ok_or("observeSubagents must be boolean")?;
        }
        Ok(())
    }

    pub fn observing(&self) -> Option<domain::observer::Observing> {
        (!self.observers.is_empty()).then(|| domain::observer::Observing {
            observers: self.observers.clone(),
            message: self.observer_message.clone(),
            subagents: !self.unobserved_subagents,
            depth: 0,
        })
    }
    pub fn allows(&self, name: &str) -> bool {
        let matches =
            |pattern: &String| pattern.strip_suffix('*').map_or(name == pattern, |prefix| name.starts_with(prefix));
        let named = domain::ToolName::of(name);
        let keeps_memory = matches!(named, domain::ToolName::Read | domain::ToolName::Write | domain::ToolName::Edit);
        (self.tools.as_ref().is_none_or(|tools| tools.iter().any(matches)) || (self.memory.is_some() && keeps_memory))
            && !self.disallowed_tools.iter().any(matches)
            && (!matches!(named, domain::ToolName::Mcp(_))
                || self
                    .mcp_servers
                    .as_ref()
                    .is_none_or(|servers| servers.iter().any(|server| name.starts_with(&format!("mcp__{server}__")))))
    }
    pub fn model(&self, parent: &str, provider: ModelProvider) -> Result<String, String> {
        let Some(model) = &self.model else {
            return Ok(parent.into());
        };
        let routed_elsewhere = crate::engine::roles::elsewhere(model, provider);
        let provider = crate::providers::find(provider);
        let vendor = provider.and_then(|provider| llm_providers::vendor(provider.vendor()));
        if let Some(full) = vendor.and_then(|vendor| vendor.alias(model)) {
            return Ok(full.into());
        }
        match provider {
            _ if model == parent => Ok(model.clone()),
            _ if routed_elsewhere => Ok(model.clone()),
            Some(provider) if provider.serves(model) => Ok(model.clone()),
            _ => {
                Err("the agent model must be served by the current provider or name a connected one as provider:model"
                    .into())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn unsupported_features_and_bad_limits_are_explicit_errors() {
        for value in [
            json!({"hooks":{"UnknownEvent":[]}}),
            json!({"maxTurns":0}),
            json!({"maxTurns":201}),
            json!({"permissionMode":"unexpected"}),
            json!({"tools":["Bash(*)"]}),
        ] {
            assert!(AgentConfig::parse(&value).is_err(), "{value}");
        }
        let config=AgentConfig::parse(&json!({"tools":"Read, mcp__docs__*","disallowedTools":["mcp__docs__write"],"mcpServers":["docs"],"background":true})).expect("config");
        assert!(config.allows("Read") && config.allows("mcp__docs__read"));
        assert!(!config.allows("mcp__docs__write") && !config.allows("mcp__other__read"));
        assert!(config.background);
    }

    #[test]
    fn a_permission_mode_reads_in_the_front_matter_spelling_and_the_app_s_own() {
        let mode = |name: &str| {
            let config = AgentConfig::parse(&json!({ "permissionMode": name })).expect(name);
            (config.permission, config.dont_ask)
        };
        for (names, expected) in [
            (["default", "ask"], PermissionMode::Ask),
            (["acceptEdits", "edits"], PermissionMode::AcceptEdits),
            (["plan", "Plan"], PermissionMode::Plan),
            (["bypassPermissions", "bypass"], PermissionMode::Bypass),
        ] {
            for name in names {
                assert_eq!(mode(name), (Some(expected), false), "{name}");
            }
        }
        assert_eq!(mode("dontAsk"), (Some(PermissionMode::Ask), true));
        assert!(AgentConfig::parse(&json!({ "permissionMode": 3 })).is_err());
    }

    #[test]
    fn an_agent_names_its_observers_what_to_tell_them_and_whether_its_subagents_are_watched_too() {
        let config = AgentConfig::parse(&json!({
            "observer": "critic, auditor",
            "observerMessage": "Watch for unsafe code.",
            "observeSubagents": false
        }))
        .expect("config");
        let observing = config.observing().expect("observed");
        assert_eq!(observing.observers, ["critic", "auditor"]);
        assert_eq!(observing.message.as_deref(), Some("Watch for unsafe code."));
        assert!(!observing.subagents);
        let listed = AgentConfig::parse(&json!({ "observer": ["critic"] })).expect("a list works too");
        assert!(listed.observing().is_some_and(|observing| observing.subagents), "subagents are watched by default");
        assert!(AgentConfig::default().observing().is_none());
        assert!(AgentConfig::parse(&json!({ "observeSubagents": "yes" })).is_err());
    }
}
