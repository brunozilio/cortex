use domain::{Access, ModelOption};
use serde_json::Value;

use crate::catalog::{self, Call, Page, STANDARD};
use crate::request::{REASONING_PREFIXES, remove, responses_effort};
use crate::{Config, Provider as Shape, ProviderError, Wire};

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "openai"
    }
    fn name(&self) -> &'static str {
        "OpenAI API"
    }
    fn vendor(&self) -> &'static str {
        "openai"
    }
    fn access(&self) -> Access {
        Access::Key
    }
    fn rank(&self) -> u8 {
        30
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0001
    }
    fn window(&self) -> u64 {
        400_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> Wire {
        Wire::Responses
    }
    fn base(&self, _config: &Config) -> String {
        "https://api.openai.com/v1".to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        vec![
            ("accept".into(), "text/event-stream".into()),
            ("authorization".into(), format!("Bearer {}", config.api_key)),
        ]
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/responses"
    }
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
        remove(body, &["reasoning", "tool_choice"]);
        responses_effort(body, model, effort, false)
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        Call::get(self, config, "/models")
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let models = catalog::items(value, "data")?
            .iter()
            .filter(|item| is_chat_model(catalog::string(item, "id")))
            .filter_map(|item| catalog::model(self, item, &STANDARD))
            .map(|mut model| {
                let id = model.id.trim_start_matches("openai:").to_owned();
                model.label = format!("{id} (OpenAI)");
                let lower = id.to_ascii_lowercase();
                catalog::reasoning(&mut model, REASONING_PREFIXES.iter().any(|p| lower.starts_with(p)));
                model
            });
        Ok(Page { models: models.collect(), next: None })
    }
    fn finish(&self, mut models: Vec<ModelOption>) -> Vec<ModelOption> {
        models.sort_by(|a, b| a.label.cmp(&b.label));
        models
    }
}

pub fn is_chat_model(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    ![
        "embedding",
        "whisper",
        "tts",
        "transcribe",
        "dall-e",
        "moderation",
        "image",
        "realtime",
        "audio",
        "search",
        "codex",
        "babbage",
        "davinci",
    ]
    .iter()
    .any(|part| lower.contains(part))
        && ["gpt", "o1", "o3", "o4", "chatgpt"].iter().any(|prefix| lower.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_chat_models_are_kept_sorted() {
        let page = json!({"data": [{"id": "o3"}, {"id": "gpt-4o"}, {"id": "whisper-1"}, {"id": "text-embedding-3"}]});
        let models = catalog::collect(&Provider, |_| Ok::<_, ProviderError>(page.clone())).unwrap_or_default();
        let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["openai:gpt-4o", "openai:o3"]);
        assert!(models[1].capabilities.as_ref().is_some_and(|c| c.reasoning));
    }

    #[test]
    fn the_name_is_bare_and_reasoning_is_left_to_the_model() {
        let mut body = json!({"model": "openai:gpt-4o", "tool_choice": "auto"});
        crate::request::shape(&Provider, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["model"], "gpt-4o");
        assert!(body.get("reasoning").is_none() && body.get("tool_choice").is_none());
    }
}
