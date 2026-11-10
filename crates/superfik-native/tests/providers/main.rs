//! The five provider accounts on the wire: requests, catalogs, sign-in and
//! token refresh.

mod accounts;
mod catalogs;
mod copilot;
mod kiro;
mod turns;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;
use superfik_core::{Effort, ModelProvider};
use superfik_native::model::TurnRequest;
use superfik_native::transcript::Message;

fn request(id: &str) -> TurnRequest {
    TurnRequest {
        model: id.into(),
        system: "Help the user.".into(),
        messages: vec![Message::user("hello")],
        tools: Vec::new(),
        effort: Effort::High,
        max_tokens: 64000,
    }
}
fn token(account: &str) -> String {
    format!(
        "e30.{}.sig",
        URL_SAFE_NO_PAD.encode(json!({"https://api.openai.com/auth":{"chatgpt_account_id":account}}).to_string())
    )
}
fn credentials(provider: ModelProvider) -> String {
    if provider.subscription() {
        json!({"auth":{"access_token":token("account-1"),"refresh_token":"refresh-1","account_id":"account-1","expires_at":"2099-01-01T00:00:00Z"}}).to_string()
    } else {
        json!({"api_key":"sk-test-key"}).to_string()
    }
}
fn header(sent: &superfik_native::model::http::Sent, key: &str) -> Option<String> {
    sent.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone())
}
