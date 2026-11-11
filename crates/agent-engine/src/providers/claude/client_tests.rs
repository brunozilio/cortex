use domain::ModelProvider;

use crate::model::ModelError;
use crate::providers::testing::{client, effort_body, header, sent_turn};

const CLAUDE: ModelProvider = ModelProvider::new("claude-subscription");

const MESSAGES: &str = include_str!("../../../tests/fixtures/messages_tool_use.sse");

#[test]
fn a_turn_goes_to_the_messages_api_signed_in() {
    let (sent, body) = sent_turn(CLAUDE, MESSAGES);
    assert_eq!(body["model"], "model-1");
    assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
    assert!(header(&sent, "x-api-key").is_none());
    assert_eq!(header(&sent, "anthropic-beta").as_deref(), Some("claude-code-20250219,oauth-2025-04-20"));
    assert_eq!(body["system"][0]["text"], "You are Claude Code, Anthropic's official CLI for Claude.");
    assert_eq!(body["system"][1]["text"], "Help the user.");
}

#[test]
fn the_chosen_effort_reaches_an_adaptive_model() {
    let body = effort_body(CLAUDE, "claude-opus-5-5", MESSAGES);
    assert_eq!(body["output_config"]["effort"], "high");
    assert_eq!(body["thinking"]["type"], "adaptive");
}

#[test]
fn the_fallback_catalog_needs_a_valid_empty_answer() {
    let models = |status: u16, body: &str| client(CLAUDE, [(status, body.to_owned())]).0.models();
    assert_eq!(models(200, "{\"data\":[]}").expect("fallback").len(), 4);
    assert!(matches!(models(401, "{}"), Err(ModelError::Unauthorized(_))));
    assert!(models(200, "{}").is_err());
}
