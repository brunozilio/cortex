use domain::ModelOption;
use domain::model::ModelCapabilities;
use serde_json::{Value, json};

use llm_providers::{Call, Config};

pub const FALLBACK: [&str; 7] = [
    "gemini-4-argon-high",
    "gemini-3.8-flash-high",
    "gemini-3.8-flash-medium",
    "gemini-3.8-flash-low",
    "claude-opus-5-5-high",
    "claude-sonnet-5-5-high",
    "gpt-oss-120b-medium",
];

pub fn call(config: &Config) -> Call {
    Call {
        url: format!("{}/v1internal:fetchAvailableModels", config.base(&super::Provider)),
        headers: super::headers(config.access_token()),
        body: Some(json!({"project": config.account()}).to_string()),
    }
}

pub fn page(value: &Value) -> Vec<ModelOption> {
    match &value["models"] {
        Value::Object(models) => models.iter().filter_map(|(id, item)| model(id, item)).collect(),
        Value::Array(models) => models
            .iter()
            .filter_map(|item| item["id"].as_str().or_else(|| item["name"].as_str()).and_then(|id| model(id, item)))
            .collect(),
        _ => Vec::new(),
    }
}

pub fn fallback() -> Vec<ModelOption> {
    FALLBACK.iter().filter_map(|id| model(id, &Value::Null)).collect()
}

fn model(id: &str, item: &Value) -> Option<ModelOption> {
    let id = id.trim().trim_start_matches("models/");
    if id.is_empty() || id.starts_with("chat_") || id.starts_with("tab_") || id.contains("image") {
        return None;
    }
    let label = item["displayName"].as_str().map(str::trim).filter(|name| !name.is_empty()).unwrap_or(id);
    let mut model = llm_providers::catalog::option(&super::Provider, id, label.to_owned());
    let number = |key: &str| item[key].as_u64().filter(|n| *n > 0);
    model.window = number("maxTokens").or_else(|| number("inputTokenLimit"));
    model.capabilities = Some(ModelCapabilities {
        reasoning: !id.starts_with("gpt-oss"),
        max_output_tokens: number("maxOutputTokens")
            .or_else(|| number("outputTokenLimit"))
            .and_then(|n| u32::try_from(n).ok()),
        ..ModelCapabilities::default()
    });
    Some(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_providers::Tokens;

    #[test]
    fn the_catalog_is_asked_for_the_accounts_project() {
        let tokens = Tokens { refresh_token: "r".into(), account_id: "proj-1".into(), ..Tokens::bearer("tok") };
        let call = call(&Config { auth: Some(tokens), ..Config::default() });
        assert_eq!(call.url, "https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels");
        assert_eq!(call.body.as_deref(), Some(r#"{"project":"proj-1"}"#));
        assert!(call.headers.contains(&("authorization".into(), "Bearer tok".into())));
    }

    #[test]
    fn a_map_of_models_lists_ids_names_and_limits_without_the_editors_own() {
        let models = page(&json!({"models": {
            "gemini-3.8-flash-high": {"displayName": "Gemini 3.8 Flash (High)", "maxTokens": 1_048_576, "maxOutputTokens": 65_536},
            "claude-opus-5-5-high": {"displayName": "Claude Opus 5.5"},
            "chat_20706": {},
            "tab_flash_lite_preview": {},
            "gemini-3-pro-image": {},
        }}));
        let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            ["antigravity-subscription:claude-opus-5-5-high", "antigravity-subscription:gemini-3.8-flash-high"]
        );
        assert_eq!(models[1].label, "Gemini 3.8 Flash (High)");
        assert_eq!(models[1].window, Some(1_048_576));
        assert_eq!(models[1].capabilities.as_ref().and_then(|c| c.max_output_tokens), Some(65_536));
    }

    #[test]
    fn a_list_of_models_and_an_unknown_answer_are_read_too() {
        let models = page(&json!({"models": [{"name": "models/gpt-oss-120b-medium"}]}));
        assert_eq!(models[0].id, "antigravity-subscription:gpt-oss-120b-medium");
        assert_eq!(models[0].capabilities.as_ref().map(|c| c.reasoning), Some(false));
        assert!(page(&json!({})).is_empty());
        assert_eq!(fallback().len(), FALLBACK.len());
    }
}
