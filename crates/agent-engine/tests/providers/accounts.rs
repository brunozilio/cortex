//! Signing in and staying signed in: OAuth, token refresh and its limits,
//! and credentials that are refused.

use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::providers::{Config, ProviderClient, oauth};
use agent_engine::model::{ModelApi, ModelError};
use domain::ModelProvider;
use domain::provider::{ProviderKey, ProviderKeyStore};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::{credentials, header, request, token};

#[derive(Default)]
struct Saved(Mutex<String>);
impl ProviderKeyStore for Saved {
    fn load(&self) -> Result<String, String> {
        Ok(self.0.lock().expect("lock").clone())
    }
    fn save(&self, value: &str) -> Result<(), String> {
        *self.0.lock().expect("lock") = value.into();
        Ok(())
    }
}

#[test]
fn expired_subscription_rotates_tokens_and_persists_them_for_a_new_client() {
    let mut config: Value = serde_json::from_str(&credentials(ModelProvider::ChatGptSubscription)).expect("config");
    config["auth"]["expires_at"] = json!("2000-01-01T00:00:00Z");
    let store = Arc::new(Saved(Mutex::new(config.to_string())));
    let http = Arc::new(ScriptedHttp::new([
        (200, json!({"access_token":token("account-2"),"refresh_token":"refresh-2","expires_in":3600}).to_string()),
        (200, "{\"models\":[]}".into()),
        (200, "{\"models\":[]}".into()),
    ]));
    for _ in 0..2 {
        let key = ProviderKey::new(store.load().expect("load")).with_store(store.clone());
        ProviderClient::new(ModelProvider::ChatGptSubscription, http.clone(), Arc::new(key)).models().expect("models");
    }
    assert_eq!(http.sent().len(), 3, "only the first client refreshes");
    assert!(http.sent()[0].body.contains("grant_type=refresh_token"));
    let saved = Config::parse(ModelProvider::ChatGptSubscription, &store.load().expect("saved")).expect("config");
    assert_eq!(saved.auth.expect("tokens").refresh_token, "refresh-2");
}

#[test]
fn codex_401_forces_one_refresh_and_replays_the_same_request() {
    let http = Arc::new(ScriptedHttp::new([
        (401, "{}".into()),
        (200, json!({"access_token":token("account-new"),"expires_in":3600}).to_string()),
        (200, include_str!("../fixtures/openai_tool_use.sse").into()),
    ]));
    let client = ProviderClient::new(
        ModelProvider::ChatGptSubscription,
        http.clone(),
        Arc::new(ProviderKey::new(credentials(ModelProvider::ChatGptSubscription))),
    );
    client.turn(&request("openai-subscription:gpt-test"), &mut |_| {}, &AtomicBool::new(false)).expect("replayed");
    let sent = http.sent();
    assert_eq!(sent.len(), 3);
    assert_eq!(sent[0].body, sent[2].body);
    assert_eq!(header(&sent[2], "chatgpt-account-id").as_deref(), Some("account-new"));
}

#[test]
fn oauth_contract_keeps_provider_specific_redirects_scopes_and_encodings() {
    let url =
        oauth::authorize(ModelProvider::ClaudeSubscription, "challenge", "state", "http://localhost:321/callback");
    assert!(url.starts_with("https://claude.com/cai/oauth/authorize?code=true&client_id="));
    assert!(url.contains("scope=user%3Ainference"));
    let http = ScriptedHttp::new([(
        200,
        json!({"access_token":"claude-access","refresh_token":"refresh","expires_in":3600}).to_string(),
    )]);
    oauth::exchange(
        &http,
        ModelProvider::ClaudeSubscription,
        "code",
        "state",
        "verifier",
        "http://localhost:321/callback",
    )
    .expect("token");
    let sent = &http.sent()[0];
    assert_eq!(sent.url, oauth::CLAUDE_TOKEN);
    let body: Value = serde_json::from_str(&sent.body).expect("json");
    assert_eq!(body["state"], "state");
    assert_eq!(body["redirect_uri"], "http://localhost:321/callback");
    assert_eq!(oauth::account_id(&token("acct")).expect("claim"), "acct");
    assert!(oauth::account_id("invalid").is_err());
}

#[test]
fn malformed_credentials_and_endpoints_are_rejected_without_printing_tokens() {
    assert!(Config::parse(ModelProvider::ChatGptSubscription, "sk-key").is_err());
    assert!(Config::parse(ModelProvider::OpenAi, "{\"api_key\":\"sk-secret\",\"base_url\":\"file:///tmp\"}").is_err());
    let config = Config::parse(ModelProvider::OpenAi, "sk-secret").expect("config");
    assert!(!format!("{config:?}").contains("sk-secret"));
}

#[test]
fn refused_refresh_is_an_authentication_failure_and_is_not_retried_as_a_network_error() {
    let http = Arc::new(ScriptedHttp::new([(401, "{}".into()), (401, "{}".into())]));
    let client = ProviderClient::new(
        ModelProvider::ChatGptSubscription,
        http.clone(),
        Arc::new(ProviderKey::new(credentials(ModelProvider::ChatGptSubscription))),
    );
    let error = client
        .turn(&request("openai-subscription:gpt-test"), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("refresh refused");
    assert!(matches!(error, ModelError::Unauthorized(_)));
    assert_eq!(http.sent().len(), 2, "do not replay after a refused refresh");
}
