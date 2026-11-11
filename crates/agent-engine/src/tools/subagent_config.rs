//! Enforced custom-agent metadata. Unsupported execution features fail explicitly
//! instead of silently granting a broader tool/permission set.
use serde_json::Value;
use domain::{Effort, ModelProvider, PermissionMode};
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
                Some("dontAsk") => {
                    config.dont_ask = true;
                    PermissionMode::Ask
                }
                Some("default") => PermissionMode::Ask,
                Some("plan") => PermissionMode::Plan,
                Some("acceptEdits") => PermissionMode::AcceptEdits,
                Some("bypassPermissions") => PermissionMode::Bypass,
                _ => return Err("unsupported custom-agent permissionMode".into()),
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
        Ok(config)
    }
    pub fn allows(&self, name: &str) -> bool {
        let matches =
            |pattern: &String| pattern.strip_suffix('*').map_or(name == pattern, |prefix| name.starts_with(prefix));
        (self.tools.as_ref().is_none_or(|tools| tools.iter().any(matches))
            || (self.memory.is_some() && matches!(name, "Read" | "Write" | "Edit")))
            && !self.disallowed_tools.iter().any(matches)
            && (!name.starts_with("mcp__")
                || self
                    .mcp_servers
                    .as_ref()
                    .is_none_or(|servers| servers.iter().any(|server| name.starts_with(&format!("mcp__{server}__")))))
    }
    pub fn model(&self, parent: &str, provider: ModelProvider) -> Result<String, String> {
        let Some(model) = &self.model else {
            return Ok(parent.into());
        };
        let claude = matches!(provider, ModelProvider::Anthropic | ModelProvider::ClaudeSubscription);
        match model.as_str() {
            "sonnet" if claude => Ok("claude-sonnet-5-5".into()),
            "opus" if claude => Ok("claude-opus-5-5".into()),
            "haiku" if claude => Ok("claude-haiku-5-5".into()),
            _ if model == parent
                || provider == ModelProvider::Local
                || (claude && model.starts_with("claude-"))
                || (!claude
                    && (model.starts_with("gpt-")
                        || model.starts_with("o1")
                        || model.starts_with("o3")
                        || model.starts_with("o4"))) =>
            {
                Ok(model.clone())
            }
            _ => Err(
                "the custom agent model must use the current provider; cross-provider credentials are not inherited"
                    .into(),
            ),
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
}
