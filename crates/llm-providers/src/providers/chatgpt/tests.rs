use super::*;
use chrono::Duration;

fn jwt(claims: Value) -> String {
    format!("h.{}.s", URL_SAFE_NO_PAD.encode(claims.to_string()))
}

#[test]
fn a_conversation_s_cache_key_also_names_its_session() {
    let headers = Provider.turn_headers(None, &json!({"prompt_cache_key": "c-1", "input": []}));
    assert_eq!(headers, [("session_id".to_owned(), "c-1".to_owned())]);
    assert!(Provider.turn_headers(None, &json!({"input": []})).is_empty());
}

#[test]
fn the_account_comes_from_any_of_the_three_claims() {
    let nested = jwt(json!({"https://api.openai.com/auth": {"chatgpt_account_id": "a1"}}));
    assert_eq!(account_id(&nested), Ok("a1".into()));
    assert_eq!(account_id(&jwt(json!({"chatgpt_account_id": "a2"}))), Ok("a2".into()));
    assert_eq!(account_id(&jwt(json!({"organizations": [{"id": "a3"}]}))), Ok("a3".into()));
    assert!(account_id("not-a-jwt").is_err());
}

#[test]
fn the_code_is_exchanged_with_a_form_without_state() {
    let request = exchange("c", "s", "v", REDIRECT);
    assert_eq!(request.url, TOKEN);
    assert!(request.body.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann") && !request.body.contains("state"));
}

#[test]
fn a_refresh_keeps_the_account_the_answer_omits() {
    let previous = Tokens { refresh_token: "r0".into(), account_id: "acct".into(), ..Tokens::bearer("old") };
    let now = Utc::now();
    let fresh = Provider.tokens(200, r#"{"access_token":"new","expires_in":60}"#, Some(&previous), now);
    let fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((fresh.refresh_token.as_str(), fresh.account_id.as_str()), ("r0", "acct"));
    assert_eq!(fresh.expires_at, Some(now + Duration::seconds(60)));
}

#[test]
fn the_account_id_is_required() {
    let tokens = Tokens { refresh_token: "r".into(), ..Tokens::bearer("a") };
    let config = Config { auth: Some(tokens), ..Config::default() };
    assert!(config.validate(&Provider).is_err());
}

#[test]
fn codex_gets_its_instructions_effort_and_no_strict_tools() {
    let mut body = json!({"model": "openai-subscription:gpt-5", "instructions": "be brief",
        "reasoning": {"effort": "max"}, "tool_choice": "auto", "tools": [{"name": "t", "strict": true}]});
    crate::request::shape(&Provider, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
    assert!(body["instructions"].as_str().is_some_and(|s| s.ends_with("be brief")));
    assert_eq!(body["reasoning"], json!({"effort": "xhigh", "summary": "auto"}));
    assert!(body.get("tool_choice").is_none() && body["tools"][0].get("strict").is_none());
}

#[test]
fn the_catalog_reads_slugs_windows_and_levels() {
    let page = json!({"models": [{"slug": "gpt-5", "context_window": 272000,
        "supported_reasoning_levels": [{"effort": "low"}, {"effort": "high"}]}]});
    let models = catalog::collect(&Provider, |_| Ok::<_, ProviderError>(page.clone())).unwrap_or_default();
    assert_eq!(models[0].id, "openai-subscription:gpt-5");
    assert_eq!(models[0].window, Some(272000));
    let caps = models[0].capabilities.clone().unwrap_or_default();
    assert!(caps.reasoning);
    assert_eq!(caps.reasoning_levels, ["low", "high"]);
}

#[test]
fn the_tokens_and_the_account_go_in_the_headers() {
    let tokens = Tokens { account_id: "acct".into(), ..Tokens::bearer("a") };
    let config = Config { auth: Some(tokens), ..Config::default() };
    let headers = config.headers(&Provider);
    assert!(headers.contains(&("chatgpt-account-id".into(), "acct".into())));
    assert_eq!(config.base(&Provider), "https://chatgpt.com/backend-api");
}

#[test]
fn the_id_token_names_the_account_when_the_access_token_does_not() {
    let response = |id_token: Value| {
        json!({"access_token": jwt(json!({})), "id_token": jwt(id_token), "refresh_token": "r", "expires_in": 3600})
            .to_string()
    };
    let account = |body: String| Provider.tokens(200, &body, None, Utc::now()).map(|tokens| tokens.account_id);
    let nested = response(json!({"https://api.openai.com/auth": {"chatgpt_account_id": "acct-id"}}));
    assert_eq!(account(nested), Ok("acct-id".into()));
    assert_eq!(account(response(json!({"chatgpt_account_id": "acct-flat"}))), Ok("acct-flat".into()));
    assert_eq!(account(response(json!({"organizations": [{"id": "org-1"}]}))), Ok("org-1".into()));
    assert!(account(response(json!({}))).is_err(), "no account anywhere");
}
