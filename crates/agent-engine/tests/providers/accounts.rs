use crate::*;
use agent_engine::model::http::ScriptedHttp;
use agent_engine::model::client::{Config, ProviderClient};
use agent_engine::model::{ModelApi, ModelError};
use domain::provider::{ProviderKey, ProviderKeyStore};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

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
    let mut config: Value = serde_json::from_str(&credentials(CHATGPT)).expect("config");
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
    let saved = Config::parse(shape(CHATGPT), &store.load().expect("saved")).expect("config");
    assert_eq!(saved.auth.expect("tokens").refresh_token, "refresh-2");
}

#[test]
fn codex_401_forces_one_refresh_and_replays_the_same_request() {
    let http = Arc::new(ScriptedHttp::new([
        (401, "{}".into()),
        (200, json!({"access_token":token("account-new"),"expires_in":3600}).to_string()),
        (200, include_str!("../fixtures/openai_tool_use.sse").into()),
    ]));
    let client = ProviderClient::new(CHATGPT, http.clone(), Arc::new(ProviderKey::new(credentials(CHATGPT))));
    client.turn(&request("openai-subscription:gpt-test"), &mut |_| {}, &AtomicBool::new(false)).expect("replayed");
    let sent = http.sent();
    assert_eq!(sent.len(), 3);
    assert_eq!(sent[0].body, sent[2].body);
    assert_eq!(header(&sent[2], "chatgpt-account-id").as_deref(), Some("account-new"));
}

#[test]
fn malformed_credentials_and_endpoints_are_rejected_without_printing_tokens() {
    assert!(Config::parse(shape(CHATGPT), "sk-key").is_err());
    assert!(Config::parse(shape(OPENAI), "{\"api_key\":\"sk-secret\",\"base_url\":\"file:///tmp\"}").is_err());
    let config = Config::parse(shape(OPENAI), "sk-secret").expect("config");
    assert!(!format!("{config:?}").contains("sk-secret"));
}

#[test]
fn refused_refresh_is_an_authentication_failure_and_is_not_retried_as_a_network_error() {
    let http = Arc::new(ScriptedHttp::new([(401, "{}".into()), (401, "{}".into())]));
    let client = ProviderClient::new(CHATGPT, http.clone(), Arc::new(ProviderKey::new(credentials(CHATGPT))));
    let error = client
        .turn(&request("openai-subscription:gpt-test"), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("refresh refused");
    assert!(matches!(error, ModelError::Unauthorized(_)));
    assert_eq!(http.sent().len(), 2, "do not replay after a refused refresh");
}
