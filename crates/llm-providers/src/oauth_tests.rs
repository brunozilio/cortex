use super::*;

fn jwt(claims: Value) -> String {
    format!("h.{}.s", URL_SAFE_NO_PAD.encode(claims.to_string()))
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
fn openai_posts_a_form_without_state_and_claude_json_with_it() {
    let openai = exchange(ModelProvider::ChatGptSubscription, "c", "s", "v", OPENAI_REDIRECT);
    assert_eq!(openai.url, OPENAI_TOKEN);
    assert!(openai.body.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann") && !openai.body.contains("state"));
    let claude = exchange(ModelProvider::ClaudeSubscription, "c", "s", "v", "http://localhost:9/callback");
    let body: Value = serde_json::from_str(&claude.body).unwrap_or_default();
    assert_eq!((body["state"].as_str(), body["client_id"].as_str()), (Some("s"), Some(CLAUDE_CLIENT)));
}

#[test]
fn a_refresh_keeps_what_the_answer_omits() {
    let previous = Tokens {
        access_token: "old".into(),
        refresh_token: "r0".into(),
        account_id: "acct".into(),
        expires_at: None,
        idc: None,
    };
    let now = Utc::now();
    let fresh = tokens(
        ModelProvider::ChatGptSubscription,
        200,
        r#"{"access_token":"new","expires_in":60}"#,
        Some(&previous),
        now,
    );
    let fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((fresh.refresh_token.as_str(), fresh.account_id.as_str()), ("r0", "acct"));
    assert_eq!(fresh.expires_at, Some(now + Duration::seconds(60)));
    assert!(matches!(
        tokens(ModelProvider::ClaudeSubscription, 400, "{}", None, now),
        Err(ProviderError::Unauthorized(_))
    ));
}

#[test]
fn each_client_id_names_its_subscription() {
    assert_eq!(subscription_client(Some(OPENAI_CLIENT)), Some(ModelProvider::ChatGptSubscription));
    assert_eq!(subscription_client(Some(CLAUDE_CLIENT)), Some(ModelProvider::ClaudeSubscription));
    assert_eq!(subscription_client(Some("other")), None);
}
