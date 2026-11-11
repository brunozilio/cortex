use domain::ModelProvider;
use serde_json::json;

use crate::model::client::Config;
use crate::providers::testing::{RESPONSES, client, effort_body, header, sent_turn};

const OPENAI: ModelProvider = ModelProvider::new("openai");

#[test]
fn a_turn_goes_to_the_responses_api_with_the_key() {
    let (sent, body) = sent_turn(OPENAI, RESPONSES);
    assert_eq!(body["model"], "model-1");
    assert_eq!(sent.url, "https://api.openai.com/v1/responses");
    assert_eq!(header(&sent, "authorization").as_deref(), Some("Bearer sk-test-key"));
    assert!(body.get("reasoning").is_none());
}

#[test]
fn the_chosen_effort_reaches_a_reasoning_model() {
    assert_eq!(effort_body(OPENAI, "gpt-5.5", RESPONSES)["reasoning"]["effort"], "high");
}

#[test]
fn the_catalog_keeps_chat_models_sorted() {
    let page = json!({"data":[{"id":"gpt-z"},{"id":"text-embedding-3"},{"id":"gpt-a"},{"id":"gpt-codex"},{"id":"o3"},{"id":"gpt-audio"}]});
    let (client, _) = client(OPENAI, [(200, page.to_string())]);
    assert_eq!(
        client.models().expect("models").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["openai:gpt-a", "openai:gpt-z", "openai:o3"]
    );
}

#[test]
fn malformed_endpoints_are_refused_and_keys_never_printed() {
    let provider = &super::Provider;
    assert!(Config::parse(provider, "{\"api_key\":\"sk-secret\",\"base_url\":\"file:///tmp\"}").is_err());
    let config = Config::parse(provider, "sk-secret").expect("config");
    assert!(!format!("{config:?}").contains("sk-secret"));
}
