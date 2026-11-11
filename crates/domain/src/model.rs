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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<String>,
}

pub fn choose_model(models: &[ModelOption], wanted: Option<&str>) -> Option<ModelOption> {
    wanted.and_then(|wanted| models.iter().find(|model| model.is_named(wanted))).or_else(|| models.first()).cloned()
}

impl ModelOption {
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
            provider: ModelProvider::default(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ModelProvider(&'static str);

impl ModelProvider {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    pub fn named(id: &str) -> Self {
        static KNOWN: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
        let mut known = KNOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(found) = known.iter().find(|known| **known == id) {
            return Self(found);
        }
        let leaked: &'static str = Box::leak(id.to_owned().into_boxed_str());
        known.push(leaked);
        Self(leaked)
    }

    pub const fn id(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for ModelProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for ModelProvider {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for ModelProvider {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        Ok(Self::named(&id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Access {
    #[default]
    Key,
    Browser,
    Code,
}

impl Access {
    pub const fn subscription(self) -> bool {
        !matches!(self, Self::Key)
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
            served("zenith:claude-opus-5-5", ModelProvider::new("zenith")),
            served("acme:gpt-6", ModelProvider::new("acme")),
        ];
        assert_eq!(choose_model(&models, Some("gpt-6")).map(|m| m.id), Some("acme:gpt-6".into()));
        assert_eq!(choose_model(&models, Some("ANTHROPIC:claude-opus-5-5")).map(|m| m.id), Some(models[0].id.clone()));
        assert_eq!(choose_model(&models, Some("gone")).map(|m| m.id), Some(models[0].id.clone()), "the first");
        assert_eq!(choose_model(&[], Some("x")), None);
    }

    #[test]
    fn a_model_round_trips_with_its_provider() {
        let model = ModelOption::new("gpt-6", "GPT-6").served_by(ModelProvider::new("acme"));
        let json = serde_json::to_string(&model).expect("serialize");
        assert_eq!(serde_json::from_str::<ModelOption>(&json).expect("round trip"), model);
        assert!(json.contains(r#""provider":"acme""#), "{json}");
    }

    #[test]
    fn a_provider_named_at_run_time_is_the_same_id() {
        assert_eq!(ModelProvider::named("acme"), ModelProvider::named(&String::from("acme")));
        assert_eq!(ModelProvider::named("acme"), ModelProvider::new("acme"));
        assert!(Access::Browser.subscription() && !Access::Key.subscription());
    }
}
