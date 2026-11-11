use crate::*;
use agent_engine::model::ModelApi;
use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::client::ProviderClient;
use domain::ModelOption;
use domain::model::ModelCapabilities;
use domain::provider::ProviderKey;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn key() -> Arc<ProviderKey> {
    Arc::new(ProviderKey::new(
        json!({"auth": {"access_token": "gho_token"}, "base_url": "https://api.business.githubcopilot.com"})
            .to_string(),
    ))
}

fn model(id: &str, endpoints: &[&str], caps: ModelCapabilities) -> ModelOption {
    let mut model = ModelOption::new(format!("copilot-subscription:{id}"), id).served_by(COPILOT);
    model.capabilities =
        Some(ModelCapabilities { endpoints: endpoints.iter().map(|e| (*e).to_owned()).collect(), ..caps });
    model
}

#[test]
fn claude_goes_to_messages_on_the_account_api() {
    let claude = model(
        "claude-sonnet-4.5",
        &["/chat/completions", "/v1/messages"],
        ModelCapabilities { adaptive_thinking: true, ..ModelCapabilities::default() },
    );
    let http = Arc::new(ScriptedHttp::new([(200, include_str!("../fixtures/anthropic_tool_use.sse").into())]));
    let client = ProviderClient::for_model(claude, http.clone(), key());
    client
        .turn(&request("copilot-subscription:claude-sonnet-4.5"), &mut |_| {}, &AtomicBool::new(false))
        .expect("turn");
    let sent = &http.sent()[0];
    assert_eq!(sent.url, "https://api.business.githubcopilot.com/v1/messages");
    assert_eq!(header(sent, "authorization").as_deref(), Some("Bearer gho_token"));
    assert_eq!(header(sent, "anthropic-version").as_deref(), Some("2023-06-01"));
    let body: Value = serde_json::from_str(&sent.body).expect("body");
    assert_eq!(body["model"], "claude-sonnet-4.5");
    assert_eq!(body["thinking"], json!({"type": "adaptive", "display": "summarized"}));
    assert_eq!(body["output_config"], json!({"effort": "high"}), "the run's effort, as the Copilot CLI sends it");
}

#[test]
fn gpt_goes_to_responses_with_an_offered_effort() {
    let gpt = model(
        "gpt-5",
        &["/chat/completions", "/responses"],
        ModelCapabilities { reasoning_levels: vec!["low".into(), "medium".into()], ..ModelCapabilities::default() },
    );
    let http = Arc::new(ScriptedHttp::new([(200, include_str!("../fixtures/openai_tool_use.sse").into())]));
    ProviderClient::for_model(gpt, http.clone(), key())
        .turn(&request("copilot-subscription:gpt-5"), &mut |_| {}, &AtomicBool::new(false))
        .expect("turn");
    let sent = &http.sent()[0];
    assert_eq!(sent.url, "https://api.business.githubcopilot.com/responses");
    let body: Value = serde_json::from_str(&sent.body).expect("body");
    assert_eq!(body["reasoning"], json!({"effort": "medium", "summary": "auto"}));
}

#[test]
fn the_catalog_lists_what_the_picker_shows() {
    let page = json!({"data": [
        {"id": "gpt-4.1", "name": "GPT-4.1", "supported_endpoints": ["/chat/completions"],
            "capabilities": {"type": "chat", "limits": {"max_context_window_tokens": 128000}}},
        {"id": "text-embedding-3-small", "capabilities": {"type": "embeddings"}},
    ]});
    let http = Arc::new(ScriptedHttp::new([(200, page.to_string())]));
    let models = ProviderClient::new(COPILOT, http.clone(), key()).models().expect("models");
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["copilot-subscription:gpt-4.1"]);
    assert_eq!(models[0].window, Some(128000));
    let sent = &http.sent()[0];
    assert_eq!(sent.url, "https://api.business.githubcopilot.com/models");
    assert_eq!(header(sent, "copilot-integration-id").as_deref(), Some("copilot-developer-cli"));
}
