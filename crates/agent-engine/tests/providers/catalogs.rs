//! Each provider's model catalog: filtering, metadata and pagination.

use serde_json::json;
use std::sync::Arc;
use domain::ModelProvider;
use domain::provider::ProviderKey;
use agent_engine::model::ModelError;
use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::providers::ProviderClient;

use crate::credentials;

#[test]
fn openai_catalog_filters_non_chat_models_and_sorts() {
    let http = Arc::new(ScriptedHttp::new([(200,json!({"data":[{"id":"gpt-z"},{"id":"text-embedding-3"},{"id":"gpt-a"},{"id":"gpt-codex"},{"id":"o3"},{"id":"gpt-audio"}]}).to_string())]));
    let client = ProviderClient::new(ModelProvider::OpenAi, http, Arc::new(ProviderKey::new("sk-key")));
    assert_eq!(
        client.models().expect("models").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["openai:gpt-a", "openai:gpt-z", "openai:o3"]
    );
}

#[test]
fn codex_catalog_uses_account_and_client_version_and_preserves_metadata() {
    let http = Arc::new(ScriptedHttp::new([(200,json!({"models":[{"slug":"gpt-test","display_name":"GPT Test","context_window":400000,"auto_compact_token_limit":272000,"effective_context_window_percent":95,"supported_reasoning_levels":[{"effort":"high"}]}]}).to_string())]));
    let client = ProviderClient::new(
        ModelProvider::ChatGptSubscription,
        http.clone(),
        Arc::new(ProviderKey::new(credentials(ModelProvider::ChatGptSubscription))),
    );
    let models = client.models().expect("models");
    assert_eq!(models[0].id, "openai-subscription:gpt-test");
    assert_eq!(models[0].window, Some(400000));
    let caps = models[0].capabilities.as_ref().expect("caps");
    assert_eq!(caps.auto_compact_token_limit, Some(272000));
    assert_eq!(caps.effective_context_window_percent, Some(95));
    assert_eq!(caps.reasoning_levels, ["high"]);
    assert_eq!(http.sent()[0].url, "https://chatgpt.com/backend-api/codex/models?client_version=0.0.0");
}

#[test]
fn anthropic_catalog_paginates_and_detects_stuck_cursors() {
    let page = json!({"data":[{"id":"claude-1","max_input_tokens":200000,"max_tokens":4096,"capabilities":{"thinking":{"supported":true,"types":{"adaptive":{"supported":true}}}}}],"has_more":true,"last_id":"cursor-1"}).to_string();
    let http = Arc::new(ScriptedHttp::new([
        (200, page.clone()),
        (200, json!({"data":[{"id":"claude-2","display_name":"Claude 2"}],"has_more":false}).to_string()),
    ]));
    let client = ProviderClient::new(ModelProvider::Anthropic, http.clone(), Arc::new(ProviderKey::new("sk-ant-key")));
    let models = client.models().expect("pages");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].capabilities.as_ref().expect("caps").max_output_tokens, Some(4096));
    assert!(http.sent()[1].url.ends_with("after_id=cursor-1"));
    let bad = ProviderClient::new(
        ModelProvider::Anthropic,
        Arc::new(ScriptedHttp::new([(200, page.clone()), (200, page)])),
        Arc::new(ProviderKey::new("key")),
    );
    assert!(bad.models().expect_err("stuck").to_string().contains("cursor"));
}

#[test]
fn claude_fallback_requires_a_valid_empty_response() {
    let make = |status, body: &str| {
        ProviderClient::new(
            ModelProvider::ClaudeSubscription,
            Arc::new(ScriptedHttp::new([(status, body.into())])),
            Arc::new(ProviderKey::new(credentials(ModelProvider::ClaudeSubscription))),
        )
    };
    assert_eq!(make(200, "{\"data\":[]}").models().expect("fallback").len(), 4);
    assert!(matches!(make(401, "{}").models(), Err(ModelError::Unauthorized(_))));
    assert!(make(200, "{}").models().is_err());
}
