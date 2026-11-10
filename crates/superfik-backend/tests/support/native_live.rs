//! Opt-in real provider inference. Auth comes from the platform's request, or
//! from an explicitly selected private credential file. Never a model CLI.
use std::sync::Arc;
use superfik_core::{ModelOption, ModelProvider, PromptRequest};
use superfik_native::{
    ApiSource, NativeEngine,
    model::{ModelApi, ModelError, http::UreqHttp, providers::ProviderClient},
    sessions::InMemorySessions,
};
pub fn model() -> ModelOption {
    let provider = match std::env::var("SUPERFIK_LIVE_PROVIDER").unwrap_or_else(|_| "anthropic".into()).as_str() {
        "openai" => ModelProvider::OpenAi,
        "openai-subscription" => ModelProvider::ChatGptSubscription,
        "claude-subscription" => ModelProvider::ClaudeSubscription,
        "anthropic" => ModelProvider::Anthropic,
        "kiro-subscription" => ModelProvider::KiroSubscription,
        _ => panic!("SUPERFIK_LIVE_PROVIDER must name one of the native providers"),
    };
    let default = match provider {
        ModelProvider::OpenAi | ModelProvider::ChatGptSubscription => "gpt-6.1-sol",
        ModelProvider::KiroSubscription => "kiro-subscription:claude-haiku-4.5",
        _ => "claude-haiku-5-5",
    };
    ModelOption::new(std::env::var("SUPERFIK_LIVE_MODEL").unwrap_or_else(|_| default.into()), "Live native model")
        .served_by(provider)
}
struct Source;
impl ApiSource for Source {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let key = match request.credentials.model.clone().or_else(|| {
            superfik_backend::ModelProviders::system()
                .key(&superfik_core::Principal::LocalOwner, request.model.provider)
                .ok()
        }) {
            Some(key) => key,
            None => {
                let path=std::env::var_os("SUPERFIK_LIVE_CREDENTIAL_FILE").ok_or_else(||ModelError::Unauthorized("Connect the provider in Superfik, or select a private SUPERFIK_LIVE_CREDENTIAL_FILE for this opt-in test.".into()))?;
                let value = std::fs::read_to_string(path)
                    .map_err(|_| ModelError::Unauthorized("Could not read the selected credential file.".into()))?;
                superfik_core::provider::ProviderKey::new(value.trim())
            }
        };
        Ok(Arc::new(ProviderClient::for_model(request.model.clone(), Arc::new(UreqHttp), Arc::new(key))))
    }
}
pub fn engine() -> NativeEngine {
    NativeEngine::new(Arc::new(Source), Arc::new(InMemorySessions::default()))
}
