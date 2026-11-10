//! Superfik's own engine on this server (ADR 0015): which API a run's
//! model is called through, with the key the server put on the run, and
//! local models served by MLX.

use std::path::Path;
use std::sync::Arc;

use superfik_core::provider::ProviderKey;
use superfik_core::{EngineKind, ModelOption, ModelProvider, Principal, PromptRequest};
use superfik_gateway::local::Endpoints as _;
use superfik_gateway::mlx::MlxRuntime;
use superfik_gateway::server::UpstreamError;
use superfik_native::model::chat::{Chat, Endpoint, Endpoints};
use superfik_native::model::http::{Http, UreqHttp};
use superfik_native::model::providers::ProviderClient;

use superfik_native::model::{ModelApi, ModelError};
use superfik_native::sessions::FileSessions;
use superfik_native::{ApiSource, NativeEngine};

use crate::providers::ModelProviders;

pub struct NativeApis {
    http: Arc<dyn Http>,
    local: Option<Arc<dyn Endpoints>>,
}

impl NativeApis {
    pub fn new(http: Arc<dyn Http>, local: Option<Arc<dyn Endpoints>>) -> Self {
        Self { http, local }
    }
}

impl ApiSource for NativeApis {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let provider = request.model.provider;
        if provider == ModelProvider::Local {
            let local = self
                .local
                .clone()
                .ok_or_else(|| ModelError::Provider("Local models need a Mac with Apple silicon (MLX).".to_owned()))?;
            return Ok(Arc::new(Chat::new(Arc::clone(&self.http), local)));
        }
        let key = request.credentials.model.clone().ok_or_else(|| {
            ModelError::Unauthorized(format!("{} is not connected: run `superfik login`", provider.name()))
        })?;
        Ok(Arc::new(ProviderClient::for_model(request.model.clone(), Arc::clone(&self.http), Arc::new(key))))
    }
}

pub struct MlxEndpoints(pub Arc<MlxRuntime>);

impl Endpoints for MlxEndpoints {
    fn endpoint(&self, model: &str) -> Result<Endpoint, ModelError> {
        let served = self.0.endpoint(model).map_err(|error| match error {
            UpstreamError::Unavailable(why) => ModelError::Overloaded(why),
            UpstreamError::Unauthorized(why) => ModelError::Unauthorized(why),
            UpstreamError::Invalid(why) | UpstreamError::Failed(why) => ModelError::Provider(why),
        })?;
        Ok(Endpoint { base_url: served.base_url, model: served.model })
    }
}

pub fn key_for(providers: &ModelProviders, owner: &Principal, model: &ModelOption) -> Option<ProviderKey> {
    let needs = model.engine == EngineKind::Native && model.provider.needs_account();
    needs.then(|| providers.key(owner, model.provider).ok()).flatten()
}

pub fn engine(data: &Path, local: Option<Arc<MlxRuntime>>) -> NativeEngine {
    let local = local.map(|runtime| Arc::new(MlxEndpoints(runtime)) as Arc<dyn Endpoints>);
    let apis = NativeApis::new(Arc::new(UreqHttp), local);
    let engine = NativeEngine::new(Arc::new(apis), Arc::new(FileSessions::new(data.join("native-sessions"))));
    match std::env::var_os("SUPERFIK_NATIVE_WORKER") {
        Some(worker) => engine.with_worker(worker.into()),
        None => engine,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use superfik_native::model::TurnRequest;
    use superfik_native::model::http::ScriptedHttp;

    use super::*;

    struct Served;

    impl Endpoints for Served {
        fn endpoint(&self, _: &str) -> Result<Endpoint, ModelError> {
            Ok(Endpoint { base_url: "http://127.0.0.1:9".into(), model: "/m/qwen".into() })
        }
    }

    fn request(provider: ModelProvider, key: Option<&str>) -> PromptRequest {
        let model = ModelOption::new(EngineKind::Native, "m", "M").served_by(provider);
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
            effort: superfik_core::Effort::Low,
            max_tokens: 1,
        };
        let _ = api.turn(&turn, &mut |_| {}, &AtomicBool::new(false));
        http.sent().last().map(|sent| sent.url.clone()).unwrap_or_default()
    }

    #[test]
    fn each_provider_is_called_through_its_own_api() {
        let http = Arc::new(ScriptedHttp::new((0..3).map(|_| (400, "{}".to_owned()))));
        let apis = NativeApis::new(http.clone(), Some(Arc::new(Served)));
        let anthropic = calls(&apis, &http, &request(ModelProvider::Anthropic, Some("sk-ant")));
        let openai = calls(&apis, &http, &request(ModelProvider::OpenAi, Some("sk")));
        let local = calls(&apis, &http, &request(ModelProvider::Local, None));
        assert_eq!(anthropic, "https://api.anthropic.com/v1/messages");
        assert_eq!(openai, "https://api.openai.com/v1/responses");
        assert_eq!(local, "http://127.0.0.1:9/v1/chat/completions");
    }

    #[test]
    fn only_a_native_run_of_a_connected_provider_carries_a_key() {
        let providers = ModelProviders::new(
            Arc::new(crate::providers::InMemoryProviderStore::default()),
            Arc::new(crate::plugins::InMemorySecretStore::default()),
        );
        let owner = Principal::LocalOwner;
        providers.connect(&owner, ModelProvider::Anthropic, "sk-ant-0123456789abcd");
        let native = |provider| ModelOption::new(EngineKind::Native, "m", "M").served_by(provider);
        let key = key_for(&providers, &owner, &native(ModelProvider::Anthropic)).expect("key");
        assert!(key.secret().contains("sk-ant-0123456789abcd"), "the provider's credential config");
        assert_eq!(key_for(&providers, &owner, &native(ModelProvider::OpenAi)), None, "not connected");
        assert_eq!(key_for(&providers, &owner, &native(ModelProvider::Local)), None, "no account");
        let other = ModelOption::new(EngineKind::Codex, "m", "M");
        assert_eq!(key_for(&providers, &owner, &other), None, "other runtimes do not receive native API credentials");
    }

    #[test]
    fn a_provider_without_a_key_or_local_models_without_mlx_say_so() {
        let apis = NativeApis::new(Arc::new(ScriptedHttp::new([])), None);
        let missing = apis.api(&request(ModelProvider::OpenAi, None)).err().expect("no key");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("OpenAI")), "{missing:?}");
        let local = apis.api(&request(ModelProvider::Local, None)).err().expect("no mlx");
        assert!(matches!(local, ModelError::Provider(ref why) if why.contains("Apple silicon")), "{local:?}");
    }
}
