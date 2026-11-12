use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use agent_engine::lsp::{Downloads, IDLE, LspPool, Processes};
use agent_engine::model::client::ProviderClient;
use agent_engine::model::http::{Http, NetHttp};
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider, Principal, PromptRequest};

use agent_engine::model::{ModelApi, ModelError};
use agent_engine::sessions::FileSessions;
use agent_engine::{ApiSource, NativeEngine};

use crate::providers::ModelProviders;

pub struct NativeApis {
    http: Arc<dyn Http>,
    catalogs: Mutex<HashMap<ModelProvider, Vec<ModelOption>>>,
}

impl NativeApis {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http, catalogs: Mutex::default() }
    }
}

impl ApiSource for NativeApis {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let provider = request.model.provider;
        let key = request.credentials.model.clone().ok_or_else(|| {
            ModelError::Unauthorized(format!(
                "{} is not connected: run `{brand_command} login`",
                agent_engine::providers::name(provider),
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

    fn catalog(&self, request: &PromptRequest) -> Vec<ModelOption> {
        let provider = request.model.provider;
        let mut catalogs = self.catalogs.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(known) = catalogs.get(&provider) {
            return known.clone();
        }
        let Some(key) = request.credentials.model.clone() else { return Vec::new() };
        let listed =
            ProviderClient::new(provider, Arc::clone(&self.http), Arc::new(key)).models().unwrap_or_else(|error| {
                tracing::warn!(%error, "could not list the provider's models to pick a small one");
                Vec::new()
            });
        catalogs.insert(provider, listed.clone());
        listed
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
    agent_engine::providers::all()
        .into_iter()
        .map(|provider| ModelProvider::named(provider.id()))
        .filter(|provider| *provider != model.provider)
        .filter_map(|provider| providers.key(owner, provider).ok().map(|key| (provider, key)))
        .collect()
}

pub fn declare_providers(home: Option<&Path>, cwd: &Path) -> Vec<String> {
    use agent_engine::declared_providers::{Loaded, load};
    let approved = agent_engine::prompt::settings::load(cwd, home, &[])
        .map(|settings| settings.enabled_providers)
        .unwrap_or_default();
    let described: Vec<(bool, String)> = load(home, cwd, &approved)
        .into_iter()
        .map(|loaded| match loaded {
            Loaded::Ready(provider) => (true, format!("{provider} loaded")),
            Loaded::Unapproved { path, id } => (
                false,
                format!(
                    "{}: the project's provider {id} waits: add it to enabledProviders in your settings",
                    path.display()
                ),
            ),
            Loaded::Broken { path, why } => (false, format!("{}: left out: {why}", path.display())),
        })
        .collect();
    for (ok, line) in &described {
        if *ok {
            tracing::info!("{line}");
        } else {
            tracing::warn!("{line}");
        }
    }
    described.into_iter().map(|(_, line)| line).collect()
}

pub const SESSIONS: &str = "native-sessions";
pub const LANGUAGE_SERVERS: &str = "lsp";

pub fn engine(data: &Path) -> NativeEngine {
    let apis = NativeApis::new(Arc::new(NetHttp));
    let servers = LspPool::installing(Processes, IDLE, data.join(LANGUAGE_SERVERS), Downloads);
    NativeEngine::new(Arc::new(apis), Arc::new(FileSessions::new(data.join(SESSIONS))))
        .with_language_servers(Arc::new(servers))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use domain::ModelProvider;

    use agent_engine::model::TurnRequest;
    use agent_engine::model::http::ScriptedHttp;

    use super::*;

    const KEYED: ModelProvider = ModelProvider::new("keyed");
    const KEYED_TOO: ModelProvider = ModelProvider::new("keyed-too");

    fn request(provider: ModelProvider, key: Option<&str>) -> PromptRequest {
        llm_providers::testing::install();
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
        let first = calls(&apis, &http, &request(KEYED, Some("sk-one")));
        let second = calls(&apis, &http, &request(KEYED_TOO, Some("sk-two")));
        assert_eq!(first, "https://keyed.test/chat");
        assert_eq!(second, "https://keyed-too.test/chat");
    }

    #[test]
    fn only_a_native_run_of_a_connected_provider_carries_a_key() {
        let providers = ModelProviders::new(
            Arc::new(crate::providers::InMemoryProviderStore::default()),
            Arc::new(crate::secrets::InMemorySecretStore::default()),
        );
        let owner = Principal::LocalOwner;
        llm_providers::testing::install();
        providers.connect(&owner, KEYED, "sk-key-0123456789abcd");
        let native = |provider| ModelOption::new("m", "M").served_by(provider);
        let key = key_for(&providers, &owner, &native(KEYED)).expect("key");
        assert!(key.secret().contains("sk-key-0123456789abcd"), "the provider's credential config");
        assert_eq!(key_for(&providers, &owner, &native(KEYED_TOO)), None, "not connected");
    }

    #[test]
    fn a_provider_prefixed_model_goes_to_that_provider_with_its_own_key() {
        let http = Arc::new(ScriptedHttp::new((0..3).map(|_| (400, "{}".to_owned()))));
        let apis = NativeApis::new(http.clone());
        let mut run = request(KEYED, Some("sk-one"));
        run.credentials.other_models = vec![(KEYED_TOO, ProviderKey::new("sk-two"))];
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
        let _ = ask("keyed-too:model-b");
        let sent = http.sent();
        assert_eq!(sent[0].url, "https://keyed-too.test/chat");
        assert!(sent[0].body.contains("\"model-b\""), "the prefix is not sent: {}", sent[0].body);
        let _ = ask("model-a");
        assert_eq!(http.sent()[1].url, "https://keyed.test/chat", "no prefix stays on the run's provider");
        let missing = ask("device-sub:auto").expect_err("not connected");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("Device")), "{missing:?}");
    }

    #[test]
    fn a_provider_s_catalog_is_listed_once_and_only_with_its_key() {
        let page = r#"{"data":[{"id":"big"},{"id":"tiny"}]}"#.to_owned();
        let http = Arc::new(ScriptedHttp::new([(200, page)]));
        let apis = NativeApis::new(http.clone());
        assert!(apis.catalog(&request(KEYED, None)).is_empty(), "no key, no listing");
        let ids = |listed: Vec<ModelOption>| listed.into_iter().map(|m| m.id).collect::<Vec<_>>();
        assert_eq!(ids(apis.catalog(&request(KEYED, Some("sk-one")))), ["keyed:big", "keyed:tiny"]);
        assert_eq!(ids(apis.catalog(&request(KEYED, Some("sk-one")))), ["keyed:big", "keyed:tiny"]);
        assert_eq!(http.sent().len(), 1, "the second run reads the cache");
    }

    #[test]
    fn a_provider_without_a_key_says_so() {
        let apis = NativeApis::new(Arc::new(ScriptedHttp::new([])));
        let missing = apis.api(&request(KEYED, None)).err().expect("no key");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("Keyed")), "{missing:?}");
    }
}
