use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::provider::{ProviderKey, ProviderKeyStore};
use domain::{ModelOption, ModelProvider};
use serde_json::{Value, json};

use crate::model::client::{Config, ProviderClient};
use crate::model::http::ScriptedHttp;
use crate::model::{ModelApi, ModelError};
use crate::providers::testing::{RESPONSES, effort_body, header, request};

const CHATGPT: ModelProvider = ModelProvider::new("openai-subscription");

fn token(account: &str) -> String {
    let claims = json!({"https://api.openai.com/auth": {"chatgpt_account_id": account}});
    format!("e30.{}.sig", URL_SAFE_NO_PAD.encode(claims.to_string()))
}

fn credentials() -> String {
    json!({"auth": {"access_token": token("account-1"), "refresh_token": "refresh-1", "account_id": "account-1", "expires_at": "2099-01-01T00:00:00Z"}}).to_string()
}

fn client(answers: impl IntoIterator<Item = (u16, String)>) -> (ProviderClient, Arc<ScriptedHttp>) {
    let http = Arc::new(ScriptedHttp::new(answers));
    (ProviderClient::new(CHATGPT, http.clone(), Arc::new(ProviderKey::new(credentials()))), http)
}

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
fn a_turn_goes_to_the_codex_backend_for_the_account() {
    let (client, http) = client([(200, RESPONSES.to_owned())]);
    client.turn(&request("openai-subscription:model-1"), &mut |_| {}, &AtomicBool::new(false)).expect("stream");
    let sent = http.sent().remove(0);
    let body: Value = serde_json::from_str(&sent.body).expect("body");
    assert_eq!(body["model"], "model-1");
    assert_eq!(sent.url, "https://chatgpt.com/backend-api/codex/responses");
    assert_eq!(header(&sent, "chatgpt-account-id").as_deref(), Some("account-1"));
    assert_eq!(header(&sent, "originator").as_deref(), Some("codex_cli_rs"));
    assert_eq!(body["reasoning"]["effort"], "high");
    assert_eq!(body["text"]["verbosity"], "medium");
    assert_eq!(body["store"], false);
    assert!(body.get("max_output_tokens").is_none());
}

#[test]
fn the_chosen_effort_reaches_a_reasoning_model() {
    assert_eq!(effort_body(CHATGPT, "gpt-5.5", RESPONSES)["reasoning"]["effort"], "high");
}

#[test]
fn an_effort_outside_the_catalog_fails_before_sending() {
    let http = Arc::new(ScriptedHttp::new([]));
    let mut model = ModelOption::new("openai-subscription:gpt-test", "Test").served_by(CHATGPT);
    model.capabilities = Some(domain::model::ModelCapabilities {
        reasoning: true,
        reasoning_levels: vec!["low".into()],
        ..Default::default()
    });
    let client = ProviderClient::for_model(model, http.clone(), Arc::new(ProviderKey::new(credentials())));
    let error = client.turn(&request("gpt-test"), &mut |_| {}, &AtomicBool::new(false)).expect_err("unsupported high");
    assert!(error.to_string().contains("does not support"));
    assert!(http.sent().is_empty());
}

#[test]
fn the_catalog_uses_the_account_and_keeps_model_metadata() {
    let page = json!({"models":[{"slug":"gpt-test","display_name":"GPT Test","context_window":400000,"auto_compact_token_limit":272000,"effective_context_window_percent":95,"supported_reasoning_levels":[{"effort":"high"}]}]});
    let (client, http) = client([(200, page.to_string())]);
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
fn expired_tokens_rotate_once_and_are_kept_for_a_new_client() {
    let mut config: Value = serde_json::from_str(&credentials()).expect("config");
    config["auth"]["expires_at"] = json!("2000-01-01T00:00:00Z");
    let store = Arc::new(Saved(Mutex::new(config.to_string())));
    let http = Arc::new(ScriptedHttp::new([
        (200, json!({"access_token":token("account-2"),"refresh_token":"refresh-2","expires_in":3600}).to_string()),
        (200, "{\"models\":[]}".into()),
        (200, "{\"models\":[]}".into()),
    ]));
    for _ in 0..2 {
        let key = ProviderKey::new(store.load().expect("load")).with_store(store.clone());
        ProviderClient::new(CHATGPT, http.clone(), Arc::new(key)).models().expect("models");
    }
    assert_eq!(http.sent().len(), 3, "only the first client refreshes");
    assert!(http.sent()[0].body.contains("grant_type=refresh_token"));
    let saved = Config::parse(&super::Provider, &store.load().expect("saved")).expect("config");
    assert_eq!(saved.auth.expect("tokens").refresh_token, "refresh-2");
}

#[test]
fn a_401_forces_one_refresh_and_replays_the_same_request() {
    let refreshed = json!({"access_token":token("account-new"),"expires_in":3600}).to_string();
    let (client, http) = client([(401, "{}".into()), (200, refreshed), (200, RESPONSES.to_owned())]);
    client.turn(&request("openai-subscription:gpt-test"), &mut |_| {}, &AtomicBool::new(false)).expect("replayed");
    let sent = http.sent();
    assert_eq!(sent.len(), 3);
    assert_eq!(sent[0].body, sent[2].body);
    assert_eq!(header(&sent[2], "chatgpt-account-id").as_deref(), Some("account-new"));
}

#[test]
fn a_refused_refresh_is_an_authentication_failure_and_not_retried() {
    let (client, http) = client([(401, "{}".into()), (401, "{}".into())]);
    let error = client
        .turn(&request("openai-subscription:gpt-test"), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("refresh refused");
    assert!(matches!(error, ModelError::Unauthorized(_)));
    assert_eq!(http.sent().len(), 2, "do not replay after a refused refresh");
}

#[test]
fn a_bare_key_is_not_a_subscription() {
    assert!(Config::parse(&super::Provider, "sk-key").is_err());
}
