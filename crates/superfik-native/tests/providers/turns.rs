//! A model turn on each provider: endpoint, authentication, body, effort
//! and the streamed reply folded.

use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use superfik_core::provider::ProviderKey;
use superfik_core::{EngineKind, ModelOption, ModelProvider};
use superfik_native::model::ModelApi;
use superfik_native::model::http::ScriptedHttp;
use superfik_native::model::providers::ProviderClient;

use crate::{credentials, header, request};

#[test]
fn four_providers_use_their_own_authentication_endpoint_and_wire_body() {
    for provider in ModelProvider::ACCOUNTS {
        let openai = matches!(provider, ModelProvider::OpenAi | ModelProvider::ChatGptSubscription);
        let fixture = if openai {
            include_str!("../fixtures/openai_tool_use.sse")
        } else {
            include_str!("../fixtures/anthropic_tool_use.sse")
        };
        let http = Arc::new(ScriptedHttp::new([(200, fixture.into())]));
        let client = ProviderClient::new(provider, http.clone(), Arc::new(ProviderKey::new(credentials(provider))));
        client
            .turn(&request(&format!("{}:model-1", provider.id())), &mut |_| {}, &AtomicBool::new(false))
            .expect("stream");
        let sent = http.sent();
        let sent = &sent[0];
        let body: Value = serde_json::from_str(&sent.body).expect("body");
        assert_eq!(body["model"], "model-1");
        match provider {
            ModelProvider::OpenAi => {
                assert_eq!(sent.url, "https://api.openai.com/v1/responses");
                assert_eq!(header(sent, "authorization").as_deref(), Some("Bearer sk-test-key"));
                assert!(body.get("reasoning").is_none());
            }
            ModelProvider::Anthropic => {
                assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
                assert_eq!(header(sent, "x-api-key").as_deref(), Some("sk-test-key"));
                assert!(header(sent, "authorization").is_none());
                assert_eq!(body["max_tokens"], 8192);
                assert_eq!(body["system"], "Help the user.");
                assert!(body.get("thinking").is_none());
            }
            ModelProvider::ChatGptSubscription => {
                assert_eq!(sent.url, "https://chatgpt.com/backend-api/codex/responses");
                assert_eq!(header(sent, "chatgpt-account-id").as_deref(), Some("account-1"));
                assert_eq!(header(sent, "originator").as_deref(), Some("codex_cli_rs"));
                assert_eq!(body["reasoning"]["effort"], "high");
                assert_eq!(body["text"]["verbosity"], "medium");
                assert_eq!(body["store"], false);
                assert!(body.get("max_output_tokens").is_none());
            }
            ModelProvider::ClaudeSubscription => {
                assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
                assert!(header(sent, "x-api-key").is_none());
                assert_eq!(header(sent, "anthropic-beta").as_deref(), Some("claude-code-20250219,oauth-2025-04-20"));
                assert_eq!(body["system"][0]["text"], "You are Claude Code, Anthropic's official CLI for Claude.");
                assert_eq!(body["system"][1]["text"], "Help the user.");
            }
            ModelProvider::Local => unreachable!(),
        }
    }
}

#[test]
fn anthropic_request_uses_the_discovered_model_capabilities() {
    let mut model = ModelOption::new(EngineKind::Native, "anthropic:claude-test", "Claude");
    model.capabilities = Some(superfik_core::model::ModelCapabilities {
        reasoning: true,
        adaptive_thinking: false,
        max_output_tokens: Some(3000),
        ..Default::default()
    });
    let http = Arc::new(ScriptedHttp::new([(200, include_str!("../fixtures/anthropic_tool_use.sse").into())]));
    let client = ProviderClient::for_model(model, http.clone(), Arc::new(ProviderKey::new("sk-ant-key")));
    client.turn(&request("anthropic:claude-test"), &mut |_| {}, &AtomicBool::new(false)).expect("stream");
    let body: Value = serde_json::from_str(&http.sent()[0].body).expect("json");
    assert_eq!(body["max_tokens"], 3000);
    assert_eq!(body["thinking"], json!({"type":"enabled","budget_tokens":1500}));
}

#[test]
fn codex_done_only_events_and_final_output_are_folded_without_duplicates() {
    use superfik_native::model::openai::Folder;
    use superfik_native::transcript::Block;
    let mut folder = Folder::new();
    let mut observed = Vec::new();
    folder
        .event(&json!({"type":"response.output_text.done","text":"done-only"}).to_string(), &mut |_| {})
        .expect("done text");
    folder
        .event(
            &json!({"type":"response.done","response":{"output":[],"usage":{"input_tokens":12,"output_tokens":2}}})
                .to_string(),
            &mut |delta| observed.push(delta),
        )
        .expect("done");
    assert_eq!(observed, vec![superfik_native::model::Delta::Text("done-only".into())]);
    assert_eq!(folder.finish().expect("reply").message.blocks, vec![Block::Text("done-only".into())]);
    let item = json!({"id":"fc1","type":"function_call","call_id":"call1","name":"Read","arguments":"{}"});
    let mut folder = Folder::new();
    folder.event(&json!({"type":"response.output_item.done","item":item}).to_string(), &mut |_| {}).expect("item");
    folder
        .event(&json!({"type":"response.completed","response":{"output":[item],"usage":{}}}).to_string(), &mut |_| {})
        .expect("completed");
    assert_eq!(folder.finish().expect("reply").message.tool_uses().count(), 1);
}

#[test]
fn chosen_effort_reaches_each_provider_for_supported_models() {
    for provider in ModelProvider::ACCOUNTS {
        let openai = matches!(provider, ModelProvider::OpenAi | ModelProvider::ChatGptSubscription);
        let id = if openai { "gpt-5.5" } else { "claude-opus-5-5" };
        let http = Arc::new(ScriptedHttp::new([(
            200,
            if openai {
                include_str!("../fixtures/openai_tool_use.sse")
            } else {
                include_str!("../fixtures/anthropic_tool_use.sse")
            }
            .into(),
        )]));
        let model = ModelOption::new(EngineKind::Native, format!("{}:{id}", provider.id()), id).served_by(provider);
        let client = ProviderClient::for_model(model, http.clone(), Arc::new(ProviderKey::new(credentials(provider))));
        client.turn(&request(id), &mut |_| {}, &AtomicBool::new(false)).expect("turn");
        let body: Value = serde_json::from_str(&http.sent()[0].body).expect("body");
        if openai {
            assert_eq!(body["reasoning"]["effort"], "high", "{provider:?}");
        } else {
            assert_eq!(body["output_config"]["effort"], "high", "{provider:?}");
            assert_eq!(body["thinking"]["type"], "adaptive");
        }
    }
}
#[test]
fn unsupported_catalog_effort_fails_before_sending_a_request() {
    let http = Arc::new(ScriptedHttp::new([]));
    let mut model = ModelOption::new(EngineKind::Native, "openai-subscription:gpt-test", "Test")
        .served_by(ModelProvider::ChatGptSubscription);
    model.capabilities = Some(superfik_core::model::ModelCapabilities {
        reasoning: true,
        reasoning_levels: vec!["low".into()],
        ..Default::default()
    });
    let client = ProviderClient::for_model(
        model,
        http.clone(),
        Arc::new(ProviderKey::new(credentials(ModelProvider::ChatGptSubscription))),
    );
    assert!(
        client
            .turn(&request("gpt-test"), &mut |_| {}, &AtomicBool::new(false))
            .expect_err("unsupported high")
            .to_string()
            .contains("does not support")
    );
    assert!(http.sent().is_empty());
}
