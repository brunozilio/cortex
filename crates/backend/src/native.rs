use std::path::Path;
use std::sync::Arc;

use agent_engine::model::http::{Http, NetHttp};
use agent_engine::model::providers::ProviderClient;
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider, Principal, PromptRequest};

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
        let main = Arc::new(ProviderClient::for_model(request.model.clone(), Arc::clone(&self.http), Arc::new(key)));
        Ok(Arc::new(agent_engine::model::routed::Routed::new(
            main,
            provider,
            Arc::clone(&self.http),
            request.credentials.other_models.clone(),
        )))
    }
}

pub fn key_for(providers: &ModelProviders, owner: &Principal, model: &ModelOption) -> Option<ProviderKey> {
    providers.key(owner, model.provider).ok()
}

pub fn other_keys(
    providers: &ModelProviders,
    owner: &Principal,
    model: &ModelOption,
) -> Vec<(ModelProvider, ProviderKey)> {
    llm_providers::all()
        .into_iter()
        .map(|provider| ModelProvider::named(provider.id()))
        .filter(|provider| *provider != model.provider)
        .filter_map(|provider| providers.key(owner, provider).ok().map(|key| (provider, key)))
        .collect()
}

pub fn declare_providers(home: Option<&Path>, cwd: &Path) {
    use agent_engine::declared_providers::{Loaded, load};
    let approved = agent_engine::prompt::settings::load(cwd, home, &[])
        .map(|settings| settings.enabled_providers)
        .unwrap_or_default();
    for loaded in load(home, cwd, &approved) {
        match loaded {
            Loaded::Ready(provider) => tracing::info!(%provider, "declared provider loaded"),
            Loaded::Unapproved { path, id } => tracing::warn!(
                path = %path.display(),
                "the project's provider {id} waits for approval: add it to enabledProviders in your own settings"
            ),
            Loaded::Broken { path, why } => {
                tracing::warn!(path = %path.display(), %why, "a declared provider was left out")
            }
        }
    }
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
    fn a_provider_prefixed_model_goes_to_that_provider_with_its_own_key() {
        let http = Arc::new(ScriptedHttp::new((0..3).map(|_| (400, "{}".to_owned()))));
        let apis = NativeApis::new(http.clone());
        let mut run = request(ModelProvider::new("anthropic"), Some("sk-ant"));
        run.credentials.other_models = vec![(ModelProvider::new("openai"), ProviderKey::new("sk-openai"))];
        let api = apis.api(&run).expect("api");
        let ask = |model: &str| {
            let turn = TurnRequest {
                model: model.into(),
                system: String::new(),
                messages: Vec::new(),
                tools: Vec::new(),
                effort: domain::Effort::Low,
                max_tokens: 1,
            };
            api.turn(&turn, &mut |_| {}, &AtomicBool::new(false))
        };
        let _ = ask("openai:gpt-6.1-sol");
        let sent = http.sent();
        assert_eq!(sent[0].url, "https://api.openai.com/v1/responses");
        assert!(sent[0].body.contains("\"gpt-6.1-sol\""), "the prefix is not sent: {}", sent[0].body);
        let _ = ask("claude-haiku-5-5");
        assert_eq!(
            http.sent()[1].url,
            "https://api.anthropic.com/v1/messages",
            "no prefix stays on the run's provider"
        );
        let missing = ask("kiro-subscription:auto").expect_err("not connected");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("Kiro")), "{missing:?}");
    }

    #[test]
    fn a_provider_without_a_key_says_so() {
        let apis = NativeApis::new(Arc::new(ScriptedHttp::new([])));
        let missing = apis.api(&request(ModelProvider::new("openai"), None)).err().expect("no key");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("OpenAI")), "{missing:?}");
    }
}
