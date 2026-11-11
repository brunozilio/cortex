use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider};
use serde_json::{Value, json};

use crate::model::ModelApi;
use crate::model::client::ProviderClient;
use crate::model::http::ScriptedHttp;
use crate::providers::testing::{MESSAGES, client, effort_body, header, request, sent_turn};

const ANTHROPIC: ModelProvider = ModelProvider::new("anthropic");

#[test]
fn a_turn_goes_to_the_messages_api_with_the_key() {
    let (sent, body) = sent_turn(ANTHROPIC, MESSAGES);
    assert_eq!(body["model"], "model-1");
    assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(header(&sent, "x-api-key").as_deref(), Some("sk-test-key"));
    assert!(header(&sent, "authorization").is_none());
    assert_eq!(body["max_tokens"], 8192);
    assert_eq!(body["system"], "Help the user.");
    assert!(body.get("thinking").is_none());
}

#[test]
fn the_chosen_effort_reaches_an_adaptive_model() {
    let body = effort_body(ANTHROPIC, "claude-opus-5-5", MESSAGES);
    assert_eq!(body["output_config"]["effort"], "high");
    assert_eq!(body["thinking"]["type"], "adaptive");
}

#[test]
fn a_request_uses_the_discovered_model_capabilities() {
    let mut model = ModelOption::new("anthropic:claude-test", "Claude").served_by(ANTHROPIC);
    model.capabilities = Some(domain::model::ModelCapabilities {
        reasoning: true,
        adaptive_thinking: false,
        max_output_tokens: Some(3000),
        ..Default::default()
    });
    let http = Arc::new(ScriptedHttp::new([(200, MESSAGES.to_owned())]));
    let client = ProviderClient::for_model(model, http.clone(), Arc::new(ProviderKey::new("sk-ant-key")));
    client.turn(&request("anthropic:claude-test"), &mut |_| {}, &AtomicBool::new(false)).expect("stream");
    let body: Value = serde_json::from_str(&http.sent()[0].body).expect("json");
    assert_eq!(body["max_tokens"], 3000);
    assert_eq!(body["thinking"], json!({"type":"enabled","budget_tokens":1500}));
}

#[test]
fn the_catalog_pages_and_notices_a_stuck_cursor() {
    let page = json!({"data":[{"id":"claude-1","max_input_tokens":200000,"max_tokens":4096,"capabilities":{"thinking":{"supported":true,"types":{"adaptive":{"supported":true}}}}}],"has_more":true,"last_id":"cursor-1"}).to_string();
    let last = json!({"data":[{"id":"claude-2","display_name":"Claude 2"}],"has_more":false}).to_string();
    let (paged, http) = client(ANTHROPIC, [(200, page.clone()), (200, last)]);
    let models = paged.models().expect("pages");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].capabilities.as_ref().expect("caps").max_output_tokens, Some(4096));
    assert!(http.sent()[1].url.ends_with("after_id=cursor-1"));
    let (stuck, _) = client(ANTHROPIC, [(200, page.clone()), (200, page)]);
    assert!(stuck.models().expect_err("stuck").to_string().contains("cursor"));
}
