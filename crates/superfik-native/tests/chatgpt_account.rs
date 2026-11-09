//! Where a ChatGPT sign-in finds the account its requests name.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use superfik_core::ModelProvider;
use superfik_native::model::http::ScriptedHttp;
use superfik_native::model::providers::oauth;

fn claims(claims: Value) -> String {
    format!("e30.{}.sig", URL_SAFE_NO_PAD.encode(claims.to_string()))
}

fn response(id_token: Value) -> String {
    json!({"access_token":claims(json!({})),"id_token":claims(id_token),"refresh_token":"r","expires_in":3600})
        .to_string()
}

fn exchange(body: String) -> Result<oauth::Tokens, superfik_native::model::ModelError> {
    oauth::exchange(
        &ScriptedHttp::new([(200, body)]),
        ModelProvider::ChatGptSubscription,
        "code",
        "state",
        "verifier",
        oauth::OPENAI_REDIRECT,
    )
}

#[test]
fn the_id_token_names_the_account_when_the_access_token_does_not() {
    let nested = response(json!({"https://api.openai.com/auth":{"chatgpt_account_id":"acct-id"}}));
    assert_eq!(exchange(nested).expect("tokens").account_id, "acct-id");
    let flat = response(json!({"chatgpt_account_id":"acct-flat"}));
    assert_eq!(exchange(flat).expect("tokens").account_id, "acct-flat");
    let organization = response(json!({"organizations":[{"id":"org-1"}]}));
    assert_eq!(exchange(organization).expect("tokens").account_id, "org-1");
    assert!(exchange(response(json!({}))).is_err(), "no account anywhere");
}

#[test]
fn a_refresh_without_an_account_keeps_the_one_it_had() {
    let previous = oauth::Tokens {
        access_token: "old".into(),
        refresh_token: "r".into(),
        account_id: "acct-kept".into(),
        expires_at: None,
    };
    let body = json!({"access_token":claims(json!({})),"expires_in":3600}).to_string();
    let tokens = oauth::refresh(&ScriptedHttp::new([(200, body)]), ModelProvider::ChatGptSubscription, &previous)
        .expect("refreshed");
    assert_eq!(tokens.account_id, "acct-kept");
}
