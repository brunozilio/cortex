//! Kiro on the wire: a turn on the profile's runtime with its ARN, a
//! rejected token refreshed once, and the catalog from the management
//! service. The streams are recorded from Kiro (event-stream frames).

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use domain::ModelProvider;
use domain::provider::{ProviderKey, ProviderKeyStore};
use agent_engine::model::http::{Http, Response, Sent};
use agent_engine::model::providers::ProviderClient;
use agent_engine::model::{ModelApi, ModelError};
use agent_engine::transcript::Block;

use crate::request;

const KIRO: ModelProvider = ModelProvider::KiroSubscription;
const ARN: &str = "arn:aws:codewhisperer:eu-central-1:699475941385:profile/EHGA3GRVQMUK";
const TOOL_USE: &[u8] = include_bytes!("../../src/model/kiro/testdata/tool_use.bin");

#[derive(Default)]
struct Wire {
    answers: Mutex<VecDeque<(u16, Vec<u8>)>>,
    sent: Mutex<Vec<Sent>>,
}

impl Wire {
    fn new(answers: impl IntoIterator<Item = (u16, Vec<u8>)>) -> Arc<Self> {
        Arc::new(Self { answers: Mutex::new(answers.into_iter().collect()), sent: Mutex::default() })
    }

    fn sent(&self) -> Vec<Sent> {
        self.sent.lock().map(|sent| sent.clone()).unwrap_or_default()
    }
}

impl Http for Wire {
    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<Response, String> {
        let sent = Sent { url: url.into(), headers: headers.to_vec(), body: body.into() };
        self.sent.lock().map_err(|e| e.to_string())?.push(sent);
        let (status, bytes) =
            self.answers.lock().map_err(|e| e.to_string())?.pop_front().ok_or("no scripted answer left")?;
        Ok(Response { status, headers: Vec::new(), body: Box::new(Cursor::new(bytes)) })
    }
}

fn key() -> Arc<ProviderKey> {
    let tokens = json!({"access_token": "tok-1", "refresh_token": "ref-1", "account_id": ARN, "expires_at": "2099-01-01T00:00:00Z"});
    Arc::new(ProviderKey::new(json!({"auth": tokens}).to_string()))
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
}

#[test]
fn a_turn_goes_to_the_profiles_runtime_and_folds_the_tool_call() {
    let wire = Wire::new([(200, TOOL_USE.to_vec())]);
    let client = ProviderClient::new(KIRO, wire.clone(), key());
    let reply = client
        .turn(&request("kiro-subscription:claude-haiku-4.5"), &mut |_| {}, &AtomicBool::new(false))
        .expect("turn");
    assert!(matches!(&reply.message.blocks[..], [Block::ToolUse { name, .. }] if name == "Read"));
    let Sent { url, headers, body } = &wire.sent()[0];
    assert_eq!(url, "https://runtime.eu-central-1.kiro.dev/");
    assert_eq!(header(headers, "authorization"), Some("Bearer tok-1"));
    assert_eq!(header(headers, "x-amz-target"), Some("AmazonCodeWhispererStreamingService.GenerateAssistantResponse"));
    assert_eq!(header(headers, "content-type"), Some("application/x-amz-json-1.0"));
    let body: Value = serde_json::from_str(body).expect("body");
    assert_eq!(body["profileArn"], ARN);
    let current = &body["conversationState"]["currentMessage"]["userInputMessage"];
    assert_eq!(current["modelId"], "claude-haiku-4.5");
    assert_eq!(current["content"], "Help the user.\n\nhello");
}

#[test]
fn a_rejected_token_is_refreshed_once_and_the_turn_replayed() {
    let denied = br#"{"__type":"com.amazon.aws.codewhisperer#AccessDeniedException","message":"The bearer token included in the request is invalid."}"#;
    let refreshed = json!({"accessToken": "tok-2", "refreshToken": "ref-2", "profileArn": ARN, "expiresIn": 3600});
    let wire = Wire::new([(403, denied.to_vec()), (200, refreshed.to_string().into_bytes()), (200, TOOL_USE.to_vec())]);
    let store = Arc::new(Saved(Mutex::new(key().secret().to_owned())));
    let key = ProviderKey::new(key().secret()).with_store(store.clone());
    let client = ProviderClient::new(KIRO, wire.clone(), Arc::new(key));
    client.turn(&request("kiro-subscription:auto"), &mut |_| {}, &AtomicBool::new(false)).expect("turn");
    let sent = wire.sent();
    assert_eq!(sent[1].url, "https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken");
    assert_eq!(serde_json::from_str::<Value>(&sent[1].body).ok(), Some(json!({"refreshToken": "ref-1"})));
    assert_eq!(header(&sent[2].headers, "authorization"), Some("Bearer tok-2"));
    assert_eq!(sent[0].body, sent[2].body, "the same turn is replayed");
    assert!(store.load().expect("saved").contains("ref-2"), "rotated tokens are kept");
}

struct Saved(Mutex<String>);

impl ProviderKeyStore for Saved {
    fn load(&self) -> Result<String, String> {
        self.0.lock().map(|saved| saved.clone()).map_err(|e| e.to_string())
    }
    fn save(&self, value: &str) -> Result<(), String> {
        *self.0.lock().map_err(|e| e.to_string())? = value.into();
        Ok(())
    }
}

#[test]
fn a_refused_refresh_signs_out() {
    let denied = br#"{"__type":"com.amazon.aws.codewhisperer#AccessDeniedException","message":"invalid"}"#;
    let wire = Wire::new([(403, denied.to_vec()), (401, br#"{"message":"Bad credentials"}"#.to_vec())]);
    let client = ProviderClient::new(KIRO, wire, key());
    let result = client.turn(&request("kiro-subscription:auto"), &mut |_| {}, &AtomicBool::new(false));
    assert!(matches!(result, Err(ModelError::Unauthorized(_))), "{result:?}");
}

#[test]
fn the_catalog_comes_from_the_management_service() {
    let page = json!({"defaultModel": {"modelId": "auto"}, "models": [
        {"modelId": "auto", "modelName": "auto", "tokenLimits": {"maxInputTokens": 1000000, "maxOutputTokens": 64000}},
        {"modelId": "claude-sonnet-4.5", "modelName": "claude-sonnet-4.5", "tokenLimits": {"maxInputTokens": 200000}}
    ]});
    let wire = Wire::new([(200, page.to_string().into_bytes())]);
    let models = ProviderClient::new(KIRO, wire.clone(), key()).models().expect("models");
    let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["kiro-subscription:auto", "kiro-subscription:claude-sonnet-4.5"]);
    assert!(models.iter().all(|m| m.provider == KIRO));
    let Sent { url, headers, body } = &wire.sent()[0];
    assert_eq!(url, "https://management.eu-central-1.kiro.dev/");
    assert_eq!(header(headers, "x-amz-target"), Some("KiroControlPlaneBearerService.ListAvailableModels"));
    assert_eq!(serde_json::from_str::<Value>(body).ok(), Some(json!({"origin": "KIRO_CLI", "profileArn": ARN})));
}
