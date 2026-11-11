use std::path::Path;
use std::sync::Arc;

use agent_engine::model::http::{Http, NetHttp};
use agent_engine::model::providers::ProviderClient;
use domain::provider::ProviderKey;
use domain::{ModelOption, Principal, PromptRequest};

use agent_engine::model::{ModelApi, ModelError};
use agent_engine::sessions::FileSessions;
use agent_engine::{ApiSource, NativeEngine};

use crate::providers::ModelProviders;

pub struct NativeApis {
    http: Arc<dyn Http>,
}

impl NativeApis {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http }
    }
}

impl ApiSource for NativeApis {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let provider = request.model.provider;
        let key = request.credentials.model.clone().ok_or_else(|| {
            ModelError::Unauthorized(format!(
                "{} is not connected: run `{brand_command} login`",
                llm_providers::name(provider),
                brand_command = brand::COMMAND
            ))
        })?;
        Ok(Arc::new(ProviderClient::for_model(request.model.clone(), Arc::clone(&self.http), Arc::new(key))))
    }
}

pub fn key_for(providers: &ModelProviders, owner: &Principal, model: &ModelOption) -> Option<ProviderKey> {
    providers.key(owner, model.provider).ok()
}

pub fn engine(data: &Path) -> NativeEngine {
    let apis = NativeApis::new(Arc::new(NetHttp));
    NativeEngine::new(Arc::new(apis), Arc::new(FileSessions::new(data.join("native-sessions"))))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use domain::ModelProvider;

    use agent_engine::model::TurnRequest;
    use agent_engine::model::http::ScriptedHttp;

    use super::*;

    fn request(provider: ModelProvider, key: Option<&str>) -> PromptRequest {
        let model = ModelOption::new("m", "M").served_by(provider);
        let mut request = PromptRequest::plain("/repo".into(), "hi", model);
        request.credentials.model = key.map(ProviderKey::new);
        request
    }

    fn calls(apis: &NativeApis, http: &ScriptedHttp, request: &PromptRequest) -> String {
        let api = apis.api(request).expect("api");
        let turn = TurnRequest {
            model: "m".into(),
            system: String::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: 1,
        };
        let _ = api.turn(&turn, &mut |_| {}, &AtomicBool::new(false));
        http.sent().last().map(|sent| sent.url.clone()).unwrap_or_default()
    }

    #[test]
    fn each_provider_is_called_through_its_own_api() {
        let http = Arc::new(ScriptedHttp::new((0..2).map(|_| (400, "{}".to_owned()))));
        let apis = NativeApis::new(http.clone());
        let anthropic = calls(&apis, &http, &request(ModelProvider::new("anthropic"), Some("sk-ant")));
        let openai = calls(&apis, &http, &request(ModelProvider::new("openai"), Some("sk")));
        assert_eq!(anthropic, "https://api.anthropic.com/v1/messages");
        assert_eq!(openai, "https://api.openai.com/v1/responses");
    }

    #[test]
    fn only_a_native_run_of_a_connected_provider_carries_a_key() {
        let providers = ModelProviders::new(
            Arc::new(crate::providers::InMemoryProviderStore::default()),
            Arc::new(crate::secrets::InMemorySecretStore::default()),
        );
        let owner = Principal::LocalOwner;
        providers.connect(&owner, ModelProvider::new("anthropic"), "sk-ant-0123456789abcd");
        let native = |provider| ModelOption::new("m", "M").served_by(provider);
        let key = key_for(&providers, &owner, &native(ModelProvider::new("anthropic"))).expect("key");
        assert!(key.secret().contains("sk-ant-0123456789abcd"), "the provider's credential config");
        assert_eq!(key_for(&providers, &owner, &native(ModelProvider::new("openai"))), None, "not connected");
    }

    #[test]
    fn a_provider_without_a_key_says_so() {
        let apis = NativeApis::new(Arc::new(ScriptedHttp::new([])));
        let missing = apis.api(&request(ModelProvider::new("openai"), None)).err().expect("no key");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("OpenAI")), "{missing:?}");
    }
}
