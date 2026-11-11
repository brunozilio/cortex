use agent_engine::model::ModelApi;
use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::providers::{ProviderClient, copilot};
use domain::model::ModelCapabilities;
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::{header, request};

const COPILOT: ModelProvider = ModelProvider::CopilotSubscription;

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

#[test]
fn the_device_flow_waits_for_approval_and_finds_the_account_api() {
    let http = ScriptedHttp::new([
        (
            200,
            json!({"device_code": "dc", "user_code": "AB12-CD34", "verification_uri": "https://github.com/login/device",
                "interval": 5, "expires_in": 900})
            .to_string(),
        ),
        (200, json!({"error": "authorization_pending"}).to_string()),
        (200, json!({"error": "slow_down", "interval": 10}).to_string()),
        (200, json!({"access_token": "gho_new", "token_type": "bearer"}).to_string()),
        (200, json!({"login": "u", "endpoints": {"api": "https://api.individual.githubcopilot.com"}}).to_string()),
    ]);
    let mut shown = Vec::new();
    let slept = Mutex::new(Vec::new());
    let config = copilot::sign_in(&http, &mut |url, code| shown.push((url, code)), &|d| {
        slept.lock().expect("lock").push(d.as_secs());
    })
    .expect("signed in");
    assert_eq!(shown, [("https://github.com/login/device".to_owned(), "AB12-CD34".to_owned())]);
    assert_eq!(*slept.lock().expect("lock"), [6, 6, 15]);
    assert_eq!(config.auth.as_ref().map(|t| t.access_token.as_str()), Some("gho_new"));
    assert_eq!(config.base(COPILOT), "https://api.individual.githubcopilot.com");
    let sent = http.sent();
    assert_eq!(sent[0].body, "client_id=Ov23ctDVkRmgkPke0Mmm&scope=read%3Auser");
    assert_eq!(header(&sent[0], "accept").as_deref(), Some("application/json"));
    assert_eq!(sent[4].url, "https://api.github.com/copilot_internal/user");
    assert_eq!(header(&sent[4], "authorization").as_deref(), Some("Bearer gho_new"));
}

#[test]
fn a_refused_or_expired_code_ends_the_sign_in() {
    let device = json!({"device_code": "dc", "user_code": "C", "verification_uri": "https://github.com/login/device",
        "interval": 5, "expires_in": 10});
    let denied = ScriptedHttp::new([
        (200, device.to_string()),
        (200, json!({"error": "access_denied", "error_description": "denied"}).to_string()),
    ]);
    let error = copilot::sign_in(&denied, &mut |_, _| {}, &|_| {}).expect_err("denied");
    assert!(error.to_string().contains("denied"));
    let pending = json!({"error": "authorization_pending"}).to_string();
    let slow = ScriptedHttp::new([(200, device.to_string()), (200, pending.clone()), (200, pending)]);
    let error = copilot::sign_in(&slow, &mut |_, _| {}, &|_| {}).expect_err("expired");
    assert!(error.to_string().contains("expired"));
}
