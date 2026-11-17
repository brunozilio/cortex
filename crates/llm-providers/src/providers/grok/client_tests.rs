use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::ModelProvider;
use domain::provider::ProviderKey;
use serde_json::{Value, json};

use crate::model::client::ProviderClient;
use crate::model::http::ScriptedHttp;
use crate::model::{ModelApi, ModelError};
use crate::providers::testing::{header, request, sent_turn};

const GROK: ModelProvider = ModelProvider::new("grok-subscription");

const RESPONSES: &str = include_str!("../../../tests/fixtures/responses_tool_use.sse");

fn credentials(expires_at: &str) -> String {
    json!({"auth": {"access_token": "access-1", "refresh_token": "refresh-1", "expires_at": expires_at}}).to_string()
}

fn client(expires_at: &str, answers: impl IntoIterator<Item = (u16, String)>) -> (ProviderClient, Arc<ScriptedHttp>) {
    let http = Arc::new(ScriptedHttp::new(answers));
    (ProviderClient::new(GROK, http.clone(), Arc::new(ProviderKey::new(credentials(expires_at)))), http)
}

#[test]
fn a_turn_goes_to_the_xai_responses_api() {
    let (sent, body) = sent_turn(GROK, RESPONSES);
    assert_eq!(sent.url, "https://api.x.ai/v1/responses");
    assert_eq!(header(&sent, "authorization").as_deref(), Some("Bearer access-1"));
    assert_eq!(body["model"], "model-1");
    assert!(body.get("reasoning").is_none(), "an unknown Grok model gets no effort dial");
    assert_eq!(body["store"], false);
}

#[test]
fn a_known_reasoning_model_gets_its_effort_alone() {
    let (client, http) = client("2099-01-01T00:00:00Z", [(200, RESPONSES.to_owned())]);
    client.turn(&request("grok-subscription:grok-4.6"), &mut |_| {}, &AtomicBool::new(false)).expect("stream");
    let body: Value = serde_json::from_str(&http.sent()[0].body).expect("body");
    assert_eq!(body["reasoning"], json!({"effort": "high"}));
}

#[test]
fn an_expired_token_is_refreshed_before_the_turn() {
    let refreshed = json!({"access_token": "access-2", "refresh_token": "refresh-2", "expires_in": 21600}).to_string();
    let (client, http) = client("2000-01-01T00:00:00Z", [(200, refreshed), (200, RESPONSES.to_owned())]);
    client.turn(&request("grok-subscription:grok-4.6"), &mut |_| {}, &AtomicBool::new(false)).expect("stream");
    let sent = http.sent();
    assert_eq!(sent[0].url, "https://auth.x.ai/oauth2/token");
    assert!(sent[0].body.contains("refresh_token=refresh-1"));
    assert_eq!(header(&sent[1], "authorization").as_deref(), Some("Bearer access-2"));
}

#[test]
fn a_401_refreshes_once_and_a_refused_refresh_is_an_authentication_failure() {
    let (client, http) = client("2099-01-01T00:00:00Z", [(401, "{}".into()), (400, "{}".into())]);
    let error = client
        .turn(&request("grok-subscription:grok-4.6"), &mut |_| {}, &AtomicBool::new(false))
        .expect_err("refresh refused");
    assert!(matches!(error, ModelError::Unauthorized(_)));
    assert_eq!(http.sent().len(), 2);
}

#[test]
fn the_catalog_is_the_xai_models_list() {
    let page = json!({"data": [{"id": "grok-4.6", "context_window": 500000}, {"id": "grok-imagine-video"}]});
    let (client, http) = client("2099-01-01T00:00:00Z", [(200, page.to_string())]);
    let models = client.models().expect("models");
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["grok-subscription:grok-4.6"]);
    assert_eq!(http.sent()[0].url, "https://api.x.ai/v1/models");
}
