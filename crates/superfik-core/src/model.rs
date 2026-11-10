//! Engines, models and reasoning effort.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineKind {
    Codex,
    Kiro,
    Copilot,
    #[serde(alias = "ClaudeCode")]
    Native,
}

impl EngineKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Kiro => "Kiro",
            Self::Copilot => "Copilot",
            Self::Native => "Superfik",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub engine: EngineKind,
    #[serde(default)]
    pub provider: ModelProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilities>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub adaptive_thinking: bool,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub auto_compact_token_limit: Option<u64>,
    #[serde(default)]
    pub effective_context_window_percent: Option<u32>,
    #[serde(default)]
    pub reasoning_levels: Vec<String>,
}

/// The model a session starts with: the one `wanted` names when offered,
/// else the first cloud model, else any.
pub fn choose_model(models: &[ModelOption], wanted: Option<&str>) -> Option<ModelOption> {
    wanted
        .and_then(|wanted| models.iter().find(|model| model.is_named(wanted)))
        .or_else(|| models.iter().find(|model| model.provider != ModelProvider::Local))
        .or_else(|| models.first())
        .cloned()
}

impl ModelOption {
    /// Whether `wanted` names this model: its id, its id without the
    /// provider prefix (`claude-opus-5-5` for `anthropic:claude-opus-5-5`),
    /// or its label, ignoring case.
    pub fn is_named(&self, wanted: &str) -> bool {
        let wanted = wanted.trim();
        let bare = self.id.split_once(':').map_or(self.id.as_str(), |(_, rest)| rest);
        self.id.eq_ignore_ascii_case(wanted)
            || bare.eq_ignore_ascii_case(wanted)
            || self.label.eq_ignore_ascii_case(wanted)
    }

    pub fn new(engine: EngineKind, id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            engine,
            provider: ModelProvider::Anthropic,
            window: None,
            capabilities: None,
        }
    }

    pub fn served_by(self, provider: ModelProvider) -> Self {
        Self { provider, ..self }
    }

    pub fn with_window(self, tokens: u64) -> Self {
        Self { window: Some(tokens), ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum ModelProvider {
    #[default]
    Anthropic,
    #[serde(alias = "Codex")]
    OpenAi,
    ChatGptSubscription,
    ClaudeSubscription,
    Local,
}

impl ModelProvider {
    pub const ACCOUNTS: [Self; 4] =
        [Self::OpenAi, Self::Anthropic, Self::ChatGptSubscription, Self::ClaudeSubscription];

    pub const fn subscription(self) -> bool {
        matches!(self, Self::ChatGptSubscription | Self::ClaudeSubscription)
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::ChatGptSubscription => "openai-subscription",
            Self::ClaudeSubscription => "claude-subscription",
            Self::Local => "local",
        }
    }

    pub const fn needs_account(self) -> bool {
        !matches!(self, Self::Local)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic API",
            Self::ChatGptSubscription => "ChatGPT Subscription",
            Self::ClaudeSubscription => "Claude Subscription",
            Self::OpenAi => "OpenAI API",
            Self::Local => "Local",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Anthropic | Self::ClaudeSubscription => "anthropic",
            Self::OpenAi | Self::ChatGptSubscription => "openai",
            Self::Local => "local",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub enum Effort {
    Low,
    #[default]
    Medium,
    High,
    XHigh,
    Max,
}

impl Effort {
    pub const ALL: [Effort; 5] = [Self::Low, Self::Medium, Self::High, Self::XHigh, Self::Max];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "Extra",
            Self::Max => "Ultracode",
        }
    }

    pub const fn cli_value(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    pub fn level(self) -> f32 {
        let index = Self::ALL.iter().position(|e| *e == self).unwrap_or(0);
        index as f32 / (Self::ALL.len() - 1) as f32
    }

    pub fn from_level(level: f32) -> Self {
        let max = (Self::ALL.len() - 1) as f32;
        let index = (level.clamp(0.0, 1.0) * max).round() as usize;
        Self::ALL[index]
    }

    pub fn is_ultra(self) -> bool {
        self == Self::Max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served(id: &str, provider: ModelProvider) -> ModelOption {
        ModelOption::new(EngineKind::Native, id, id.rsplit(':').next().unwrap_or(id)).served_by(provider)
    }

    #[test]
    fn the_asked_model_wins_then_a_cloud_one() {
        let models = [
            served("mlx/qwen", ModelProvider::Local),
            served("anthropic:claude-opus-5-5", ModelProvider::Anthropic),
            served("openai:gpt-6", ModelProvider::OpenAi),
        ];
        assert_eq!(choose_model(&models, Some("gpt-6")).map(|m| m.id), Some("openai:gpt-6".into()));
        assert_eq!(choose_model(&models, Some("ANTHROPIC:claude-opus-5-5")).map(|m| m.id), Some(models[1].id.clone()));
        assert_eq!(choose_model(&models, Some("gone")).map(|m| m.id), Some(models[1].id.clone()), "first cloud");
        assert_eq!(choose_model(&models[..1], None).map(|m| m.id), Some("mlx/qwen".into()));
        assert_eq!(choose_model(&[], Some("x")), None);
    }

    #[test]
    fn a_model_saved_before_providers_is_served_by_anthropic() {
        let old = r#"{"id":"claude-opus-5-5","label":"Opus 5.5","engine":"ClaudeCode"}"#;
        let model: ModelOption = serde_json::from_str(old).expect("old model");
        assert_eq!(model.provider, ModelProvider::Anthropic);
        let local = ModelOption::new(EngineKind::Native, "qwen", "Qwen").served_by(ModelProvider::Local);
        let json = serde_json::to_string(&local).expect("serialize");
        assert_eq!(serde_json::from_str::<ModelOption>(&json).expect("round trip"), local);
    }

    #[test]
    fn a_codex_model_saved_by_earlier_builds_is_served_by_openai() {
        let old = r#"{"id":"gpt-codex","label":"Codex","engine":"ClaudeCode","provider":"Codex"}"#;
        let model: ModelOption = serde_json::from_str(old).expect("old model");
        assert_eq!(model.provider, ModelProvider::OpenAi);
    }

    #[test]
    fn only_company_apis_need_an_account() {
        assert!(ModelProvider::ACCOUNTS.iter().all(|p| p.needs_account()));
        assert!(!ModelProvider::Local.needs_account());
    }

    #[test]
    fn effort_level_round_trips() {
        for effort in Effort::ALL {
            assert_eq!(Effort::from_level(effort.level()), effort);
        }
    }

    #[test]
    fn effort_level_is_clamped() {
        assert_eq!(Effort::from_level(-3.0), Effort::Low);
        assert_eq!(Effort::from_level(7.0), Effort::Max);
    }

    #[test]
    fn only_max_is_ultra() {
        assert!(Effort::Max.is_ultra());
        assert!(!Effort::High.is_ultra());
        assert_eq!(Effort::Max.label(), "Ultracode");
    }
}
