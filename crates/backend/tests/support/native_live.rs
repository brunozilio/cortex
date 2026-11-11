use agent_engine::{
    ApiSource, NativeEngine,
    model::{ModelApi, ModelError, http::NetHttp, providers::ProviderClient},
    sessions::InMemorySessions,
};
use domain::{ModelOption, ModelProvider, PromptRequest};
use std::sync::Arc;
pub fn model() -> ModelOption {
    let named = std::env::var(concat!(brand::env_prefix!(), "_LIVE_PROVIDER")).unwrap_or_else(|_| "anthropic".into());
    let Some(provider) = llm_providers::find(ModelProvider::named(&named)).map(llm_providers::id) else {
        panic!("{brand_env}_LIVE_PROVIDER must name one of the native providers", brand_env = brand::ENV)
    };
    let default = match provider.id() {
        "openai" | "openai-subscription" => "gpt-6.1-sol",
        "kiro-subscription" => "kiro-subscription:claude-haiku-4.5",
        _ => "claude-haiku-5-5",
    };
    ModelOption::new(
        std::env::var(concat!(brand::env_prefix!(), "_LIVE_MODEL")).unwrap_or_else(|_| default.into()),
        "Live native model",
    )
    .served_by(provider)
}
struct Source;
impl ApiSource for Source {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let key = match request.credentials.model.clone().or_else(|| {
            backend::ModelProviders::system().key(&domain::Principal::LocalOwner, request.model.provider).ok()
        }) {
            Some(key) => key,
            None => {
                let path =
                    std::env::var_os(concat!(brand::env_prefix!(), "_LIVE_CREDENTIAL_FILE")).ok_or_else(|| {
                        ModelError::Unauthorized(
                            concat!(
                                "Connect the provider in ",
                                brand::name!(),
                                ", or select a private ",
                                brand::env_prefix!(),
                                "_LIVE_CREDENTIAL_FILE for this opt-in test."
                            )
                            .into(),
                        )
                    })?;
                let value = std::fs::read_to_string(path)
                    .map_err(|_| ModelError::Unauthorized("Could not read the selected credential file.".into()))?;
                domain::provider::ProviderKey::new(value.trim())
            }
        };
        Ok(Arc::new(ProviderClient::for_model(request.model.clone(), Arc::new(NetHttp), Arc::new(key))))
    }
}
pub fn engine() -> NativeEngine {
    NativeEngine::new(Arc::new(Source), Arc::new(InMemorySessions::default()))
}
