use domain::{Access, ModelOption};
use serde_json::{Value, json};

use llm_providers::catalog::{self, Call, Page, STANDARD};
use llm_providers::request::remove;
use llm_providers::{Config, Provider as Shape, ProviderError};

pub const API: &str = "https://api.anthropic.com/v1";
pub const VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "anthropic"
    }
    fn name(&self) -> &'static str {
        "Anthropic API"
    }
    fn vendor(&self) -> &'static str {
        "anthropic"
    }
    fn access(&self) -> Access {
        Access::Key
    }
    fn rank(&self) -> u8 {
        10
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0002
    }
    fn window(&self) -> u64 {
        200_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        "messages"
    }

    fn web_search(&self, model: &str) -> Option<serde_json::Value> {
        Some(llm_providers::web_search::anthropic(model))
    }
    fn base(&self, _config: &Config) -> String {
        API.to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        vec![
            ("accept".into(), "text/event-stream".into()),
            ("x-api-key".into(), config.api_key.clone()),
            ("anthropic-version".into(), VERSION.into()),
        ]
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/messages"
    }
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
        let caps = model.and_then(|model| model.capabilities.as_ref());
        let max_tokens = crate::providers::output_limit(model, body);
        body["max_tokens"] = json!(max_tokens);
        remove(body, &["thinking", "output_config"]);
        if let Some(caps) = caps.filter(|c| c.reasoning && (c.adaptive_thinking || max_tokens > 1)) {
            body["thinking"] = if caps.adaptive_thinking {
                json!({"type":"adaptive"})
            } else {
                json!({"type":"enabled", "budget_tokens": if max_tokens <= 4096 { max_tokens / 2 } else { 4096 }})
            };
        }
        llm_providers::effort(self.vendor(), body, model, effort, false)
    }
    fn catalog(&self, config: &Config, after: &str) -> Call {
        Call::get(self, config, &models_path(after))
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let models = catalog::items(value, "data")?.iter().filter_map(|item| catalog::model(self, item, &STANDARD));
        let next = (value["has_more"] == true).then(|| catalog::string(value, "last_id").to_owned());
        Ok(Page { models: models.collect(), next })
    }
}

pub fn models_path(after: &str) -> String {
    if after.is_empty() {
        "/models?limit=1000".to_owned()
    } else {
        format!("/models?limit=1000&after_id={}", llm_providers::oauth::form(&[("v", after)]).trim_start_matches("v="))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::model::ModelCapabilities;

    #[test]
    fn the_catalog_follows_its_cursor_and_refuses_a_repeated_one() {
        let mut calls = Vec::new();
        let models = catalog::collect(&Provider, |after| {
            calls.push(models_path(after));
            Ok::<_, ProviderError>(if calls.len() == 1 {
                json!({"data": [{"id": "claude-a"}], "has_more": true, "last_id": "claude-a"})
            } else {
                json!({"data": [{"id": "claude-b"}], "has_more": false})
            })
        });
        assert_eq!(models.map(|m| m.len()), Ok(2));
        assert_eq!(calls[1], "/models?limit=1000&after_id=claude-a");
        let stuck = json!({"data": [], "has_more": true, "last_id": "x"});
        assert!(catalog::collect(&Provider, |_| Ok::<_, ProviderError>(stuck.clone())).is_err());
    }

    #[test]
    fn the_system_keeps_its_blocks_and_thinking_follows_the_capabilities() {
        let mut model = ModelOption::new("anthropic:claude-opus-4-1", "Opus");
        model.capabilities =
            Some(ModelCapabilities { reasoning: true, max_output_tokens: Some(32000), ..ModelCapabilities::default() });
        let mut body = json!({"model": "anthropic:claude-opus-4-1", "system": [{"text": "a"}, {"text": "b"}],
            "output_config": {"effort": "high"}});
        llm_providers::request::shape(&Provider, Some(&model), &mut body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["system"], json!([{"text": "a"}, {"text": "b"}]));
        assert_eq!(body["thinking"], json!({"type": "enabled", "budget_tokens": 16000}));
    }

    fn shaped(declared: Option<u32>, asked: Option<u64>) -> Value {
        let mut model = ModelOption::new("anthropic:claude-opus-4-1", "Opus");
        model.capabilities =
            Some(ModelCapabilities { reasoning: true, max_output_tokens: declared, ..ModelCapabilities::default() });
        let mut body = json!({"model": "anthropic:claude-opus-4-1", "output_config": {"effort": "max"}});
        if let Some(asked) = asked {
            body["max_tokens"] = json!(asked);
        }
        llm_providers::request::shape(&Provider, Some(&model), &mut body).unwrap_or_else(|e| panic!("{e}"));
        body
    }

    #[test]
    fn the_output_limit_is_the_smaller_of_what_the_model_declares_and_what_the_request_asks() {
        assert_eq!(shaped(Some(32_000), Some(64_000))["max_tokens"], 32_000, "the model's limit");
        assert_eq!(shaped(Some(128_000), Some(64_000))["max_tokens"], 64_000, "the request's limit");
        assert_eq!(shaped(Some(32_000), None)["max_tokens"], 32_000);
        assert_eq!(shaped(None, Some(64_000))["max_tokens"], 8192, "a model that declares nothing");
        for body in [shaped(Some(32_000), Some(64_000)), shaped(Some(128_000), Some(2_000)), shaped(None, None)] {
            let (most, thinking) = (body["max_tokens"].as_u64(), body["thinking"]["budget_tokens"].as_u64());
            assert!(thinking < most, "thinking leaves room for the answer: {body}");
        }
        assert!(shaped(Some(128_000), Some(300)).get("thinking").is_none(), "too little room to think");
    }

    #[test]
    fn a_key_goes_in_its_own_header() {
        let config = Config { api_key: "sk".into(), ..Config::default() };
        assert!(config.headers(&Provider).contains(&("x-api-key".into(), "sk".into())));
        assert_eq!(config.base(&Provider), API);
    }
}

#[cfg(test)]
mod client_tests;
