use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::provider::ProviderKey;
use domain::{Effort, ModelOption, ModelProvider};
use serde_json::{Value, json};

use crate::model::client::ProviderClient;
use crate::model::http::{ScriptedHttp, Sent};
use crate::model::{ModelApi, TurnRequest};
use crate::transcript::Message;

pub const MESSAGES: &str = include_str!("../../tests/fixtures/messages_tool_use.sse");
pub const RESPONSES: &str = include_str!("../../tests/fixtures/responses_tool_use.sse");
pub const CHAT: &str = include_str!("../../tests/fixtures/chat_tool_use.sse");

pub fn request(model: &str) -> TurnRequest {
    TurnRequest {
        model: model.into(),
        system: "Help the user.".into(),
        messages: vec![Message::user("hello")],
        tools: Vec::new(),
        effort: Effort::High,
        max_tokens: 64000,
    }
}

pub fn credentials(provider: ModelProvider) -> String {
    let subscription = super::get(provider).is_ok_and(|known| known.access().subscription());
    if subscription {
        json!({"auth": {"access_token": "access-1", "refresh_token": "refresh-1", "account_id": "account-1", "expires_at": "2099-01-01T00:00:00Z"}}).to_string()
    } else {
        json!({"api_key": "sk-test-key"}).to_string()
    }
}

pub fn header(sent: &Sent, key: &str) -> Option<String> {
    sent.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone())
}

pub fn client(
    provider: ModelProvider,
    answers: impl IntoIterator<Item = (u16, String)>,
) -> (ProviderClient, Arc<ScriptedHttp>) {
    let http = Arc::new(ScriptedHttp::new(answers));
    (ProviderClient::new(provider, http.clone(), Arc::new(ProviderKey::new(credentials(provider)))), http)
}

pub fn sent_turn(provider: ModelProvider, fixture: &str) -> (Sent, Value) {
    let (client, http) = client(provider, [(200, fixture.to_owned())]);
    client.turn(&request(&format!("{}:model-1", provider.id())), &mut |_| {}, &AtomicBool::new(false)).expect("stream");
    let sent = http.sent().remove(0);
    let body = serde_json::from_str(&sent.body).expect("body");
    (sent, body)
}

pub fn effort_body(provider: ModelProvider, model: &str, fixture: &str) -> Value {
    let http = Arc::new(ScriptedHttp::new([(200, fixture.to_owned())]));
    let option = ModelOption::new(format!("{}:{model}", provider.id()), model).served_by(provider);
    let key = Arc::new(ProviderKey::new(credentials(provider)));
    ProviderClient::for_model(option, http.clone(), key)
        .turn(&request(model), &mut |_| {}, &AtomicBool::new(false))
        .expect("turn");
    serde_json::from_str(&http.sent()[0].body).expect("body")
}
