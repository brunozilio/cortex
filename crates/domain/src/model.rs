//! Models, who serves them, and reasoning effort.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
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
    /// The API paths a provider serves the model on (GitHub Copilot's
    /// `supported_endpoints`); empty when the provider has one API.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<String>,
}

/// The model a session starts with: the one `wanted` names when offered,
/// else the first.
pub fn choose_model(models: &[ModelOption], wanted: Option<&str>) -> Option<ModelOption> {
    wanted.and_then(|wanted| models.iter().find(|model| model.is_named(wanted))).or_else(|| models.first()).cloned()
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

    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
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
    OpenAi,
    ChatGptSubscription,
    ClaudeSubscription,
    CopilotSubscription,
    KiroSubscription,
}

impl ModelProvider {
    pub const ACCOUNTS: [Self; 6] = [
        Self::OpenAi,
        Self::Anthropic,
        Self::ChatGptSubscription,
        Self::ClaudeSubscription,
        Self::CopilotSubscription,
        Self::KiroSubscription,
    ];

    pub const fn subscription(self) -> bool {
        matches!(
            self,
            Self::ChatGptSubscription | Self::ClaudeSubscription | Self::CopilotSubscription | Self::KiroSubscription
        )
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::ChatGptSubscription => "openai-subscription",
            Self::ClaudeSubscription => "claude-subscription",
            Self::CopilotSubscription => "copilot-subscription",
            Self::KiroSubscription => "kiro-subscription",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic API",
            Self::ChatGptSubscription => "ChatGPT Subscription",
            Self::ClaudeSubscription => "Claude Subscription",
            Self::CopilotSubscription => "GitHub Copilot Subscription",
            Self::KiroSubscription => "Kiro Subscription",
            Self::OpenAi => "OpenAI API",
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

    pub const fn cli_value(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served(id: &str, provider: ModelProvider) -> ModelOption {
        ModelOption::new(id, id.rsplit(':').next().unwrap_or(id)).served_by(provider)
    }

    #[test]
    fn the_asked_model_wins_then_the_first() {
        let models = [
            served("anthropic:claude-opus-5-5", ModelProvider::Anthropic),
            served("openai:gpt-6", ModelProvider::OpenAi),
        ];
        assert_eq!(choose_model(&models, Some("gpt-6")).map(|m| m.id), Some("openai:gpt-6".into()));
        assert_eq!(choose_model(&models, Some("ANTHROPIC:claude-opus-5-5")).map(|m| m.id), Some(models[0].id.clone()));
        assert_eq!(choose_model(&models, Some("gone")).map(|m| m.id), Some(models[0].id.clone()), "the first");
        assert_eq!(choose_model(&[], Some("x")), None);
    }

    #[test]
    fn a_model_round_trips_with_its_provider() {
        let model = ModelOption::new("gpt-6", "GPT-6").served_by(ModelProvider::OpenAi);
        let json = serde_json::to_string(&model).expect("serialize");
        assert_eq!(serde_json::from_str::<ModelOption>(&json).expect("round trip"), model);
    }
}
