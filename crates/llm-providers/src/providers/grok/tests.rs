use super::*;
use serde_json::json;

fn signed_in() -> Config {
    let tokens = Tokens { refresh_token: "r".into(), ..Tokens::bearer("a") };
    Config { auth: Some(tokens), ..Config::default() }
}

#[test]
fn a_subscription_needs_both_tokens() {
    assert!(signed_in().validate(&Provider).is_ok());
    let access_only = Config { auth: Some(Tokens::bearer("a")), ..Config::default() };
    assert!(access_only.validate(&Provider).is_err());
    assert!(Config::parse(&Provider, "xai-key").is_err());
}

#[test]
fn turns_go_to_the_xai_responses_api_with_the_bearer() {
    let config = signed_in();
    assert_eq!(config.base(&Provider), "https://api.x.ai/v1");
    assert!(config.headers(&Provider).contains(&("authorization".into(), "Bearer a".into())));
    assert_eq!(Provider.path(None), "/responses");
}

#[test]
fn a_refresh_is_a_form_to_the_xai_token_endpoint() {
    let request = Provider.refresh(&Tokens { refresh_token: "r0".into(), ..Tokens::bearer("a") });
    let request = request.unwrap_or_else(|| panic!("refreshable"));
    assert_eq!(request.url, "https://auth.x.ai/oauth2/token");
    assert!(request.body.contains("grant_type=refresh_token") && request.body.contains("refresh_token=r0"));
    assert!(request.body.contains("client_id=b1a00492-073a-47ea-816f-4c329264a828"));
}

#[test]
fn a_rotated_refresh_token_replaces_the_old_one() {
    let previous = Tokens { refresh_token: "r0".into(), ..Tokens::bearer("old") };
    let body = r#"{"access_token":"new","refresh_token":"r1","expires_in":21600}"#;
    let fresh = Provider.tokens(200, body, Some(&previous), Utc::now()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((fresh.access_token.as_str(), fresh.refresh_token.as_str()), ("new", "r1"));
}

#[test]
fn the_catalog_keeps_grok_chat_models_with_their_largest_window() {
    let page = json!({"data": [
        {"id": "grok-4.6", "context_windows": [256000, 500000]},
        {"id": "grok-4.20-0309-non-reasoning", "context_window": 2000000},
        {"id": "grok-imagine-image"},
        {"id": "grok-composer-2.5-fast", "supports_reasoning_effort": false},
        {"id": "other-model"}
    ]});
    let models = catalog::collect(&Provider, |_| Ok::<_, ProviderError>(page.clone())).unwrap_or_default();
    let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "grok-subscription:grok-composer-2.5-fast",
            "grok-subscription:grok-4.6",
            "grok-subscription:grok-4.20-0309-non-reasoning"
        ]
    );
    let reasons = |i: usize| models[i].capabilities.as_ref().is_some_and(|c| c.reasoning);
    assert_eq!((reasons(0), reasons(1), reasons(2)), (false, true, false));
    assert_eq!((models[1].window, models[2].window), (Some(500000), Some(2000000)));
}

#[test]
fn the_body_carries_the_bare_model_and_xai_s_effort() {
    let mut body = json!({"model": "grok-subscription:grok-4.6", "reasoning": {"effort": "high", "summary": "auto"}});
    llm_providers::request::shape(&Provider, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(body["model"], "grok-4.6");
    assert_eq!(body["reasoning"], json!({"effort": "high"}));
}
