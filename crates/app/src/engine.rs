use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_engine::lsp::{Downloads, IDLE, LspPool, Processes};
use agent_engine::model::client::ProviderClient;
use agent_engine::model::http::{Http, NetHttp};
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider, PromptRequest};
use platform::sync::Locked;

use agent_engine::model::{ModelApi, ModelError};
use agent_engine::sessions::FileSessions;
use agent_engine::{ApiSource, Engine};

use crate::providers::ModelProviders;

pub struct ProviderApis {
    http: Arc<dyn Http>,
    catalogs: Mutex<HashMap<ModelProvider, Vec<ModelOption>>>,
}

impl ProviderApis {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self { http, catalogs: Mutex::default() }
    }
}

impl ApiSource for ProviderApis {
    fn api(&self, request: &PromptRequest) -> Result<Arc<dyn ModelApi>, ModelError> {
        let provider = request.session.model.provider;
        let key = request.host.credentials.model.clone().ok_or_else(|| {
            ModelError::Unauthorized(format!(
                "{} is not connected: run `{brand_command} login`",
                agent_engine::providers::name(provider),
                brand_command = brand::COMMAND
            ))
        })?;
        let client = |key: ProviderKey| -> Arc<dyn ModelApi> {
            Arc::new(ProviderClient::for_model(request.session.model.clone(), Arc::clone(&self.http), Arc::new(key)))
        };
        let pool = &request.host.credentials.model_pool;
        let main = if pool.is_empty() {
            client(key)
        } else {
            let named = &request.host.credentials.model_accounts;
            let accounts = std::iter::once(key)
                .chain(pool.iter().cloned())
                .enumerate()
                .map(|(at, key)| (named.get(at).cloned().unwrap_or_else(|| format!("account {}", at + 1)), client(key)))
                .collect();
            Arc::new(agent_engine::model::pooled::Pooled::new(accounts))
        };
        Ok(Arc::new(agent_engine::model::routed::Routed::new(
            main,
            provider,
            Arc::clone(&self.http),
            request.host.credentials.other_models.clone(),
        )))
    }

    fn catalog(&self, request: &PromptRequest) -> Vec<ModelOption> {
        let provider = request.session.model.provider;
        let mut catalogs = self.catalogs.locked();
        if let Some(known) = catalogs.get(&provider) {
            return known.clone();
        }
        let Some(key) = request.host.credentials.model.clone() else { return Vec::new() };
        let listed =
            ProviderClient::new(provider, Arc::clone(&self.http), Arc::new(key)).models().unwrap_or_else(|error| {
                tracing::warn!(%error, "could not list the provider's models to pick a small one");
                Vec::new()
            });
        catalogs.insert(provider, listed.clone());
        listed
    }

    fn searcher(&self, request: &PromptRequest) -> Option<Arc<dyn agent_engine::model::Searcher>> {
        let key = request.host.credentials.model.clone()?;
        Some(Arc::new(ProviderClient::for_model(request.session.model.clone(), Arc::clone(&self.http), Arc::new(key))))
    }
}

pub fn keys_for(providers: &ModelProviders, model: &ModelOption) -> Vec<(String, ProviderKey)> {
    providers.keys(model.provider).unwrap_or_default()
}

pub fn other_keys(providers: &ModelProviders, model: &ModelOption) -> Vec<(ModelProvider, ProviderKey)> {
    agent_engine::providers::all()
        .into_iter()
        .map(|provider| ModelProvider::named(provider.id()))
        .filter(|provider| *provider != model.provider)
        .filter_map(|provider| providers.key(provider).ok().map(|key| (provider, key)))
        .collect()
}

pub fn declare_providers(gate: &agent_engine::trust::Gate) -> Vec<String> {
    use agent_engine::declared_providers::{Loaded, load};
    let approved =
        agent_engine::prompt::settings::load(gate, &[]).map(|settings| settings.enabled_providers).unwrap_or_default();
    let described: Vec<(bool, String)> = load(gate, &approved)
        .into_iter()
        .map(|loaded| match loaded {
            Loaded::Ready(provider) => (true, format!("{provider} loaded")),
            Loaded::Untrusted { path } => (
                false,
                format!("{}: the project's provider is skipped: {}", path.display(), domain::trust::NOT_TRUSTED),
            ),
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

pub fn build(sessions: Arc<FileSessions>, language_servers: PathBuf) -> Engine {
    let apis = ProviderApis::new(Arc::new(NetHttp));
    let servers = LspPool::installing(Processes, IDLE, language_servers, Downloads(agent_engine::NetFetch));
    Engine::new(Arc::new(apis), sessions).with_language_servers(Arc::new(servers))
}

#[cfg(test)]
mod tests {
    use domain::ModelProvider;

    use agent_engine::model::http::ScriptedHttp;
    use agent_engine::model::{BlockingTurn, CancellationToken, TurnRequest};

    use super::*;

    const KEYED: ModelProvider = ModelProvider::new("keyed");
    const KEYED_TOO: ModelProvider = ModelProvider::new("keyed-too");

    fn request(provider: ModelProvider, key: Option<&str>) -> PromptRequest {
        llm_providers::testing::install();
        let model = ModelOption::new("m", "M").served_by(provider);
        let mut request = PromptRequest::plain("/repo".into(), "hi", model);
        request.host.credentials.model = key.map(ProviderKey::new);
        request
    }

    fn calls(apis: &ProviderApis, http: &ScriptedHttp, request: &PromptRequest) -> String {
        let api = apis.api(request).expect("api");
        let turn = TurnRequest {
            model: "m".into(),
            system: String::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: 1,
        };
        let _ = api.turn_blocking(&turn, &mut |_| {}, &CancellationToken::new());
        http.sent().last().map(|sent| sent.url.clone()).unwrap_or_default()
    }

    #[test]
    fn each_provider_is_called_through_its_own_api() {
        let http = Arc::new(ScriptedHttp::new((0..2).map(|_| (400, "{}".to_owned()))));
        let apis = ProviderApis::new(http.clone());
        let first = calls(&apis, &http, &request(KEYED, Some("sk-one")));
        let second = calls(&apis, &http, &request(KEYED_TOO, Some("sk-two")));
        assert_eq!(first, "https://keyed.test/chat");
        assert_eq!(second, "https://keyed-too.test/chat");
    }

    #[test]
    fn only_a_run_of_a_connected_provider_carries_a_key() {
        let providers = ModelProviders::new(
            Arc::new(crate::providers::InMemoryProviderStore::default()),
            Arc::new(crate::secrets::InMemorySecretStore::default()),
        );
        llm_providers::testing::install();
        providers.connect(KEYED, "sk-key-0123456789abcd");
        let native = |provider| ModelOption::new("m", "M").served_by(provider);
        let (label, key) = keys_for(&providers, &native(KEYED)).remove(0);
        assert_eq!(label, "…abcd");
        assert!(key.secret().contains("sk-key-0123456789abcd"), "the provider's credential config");
        assert!(keys_for(&providers, &native(KEYED_TOO)).is_empty(), "not connected");
    }

    #[test]
    fn a_provider_prefixed_model_goes_to_that_provider_with_its_own_key() {
        let http = Arc::new(ScriptedHttp::new((0..3).map(|_| (400, "{}".to_owned()))));
        let apis = ProviderApis::new(http.clone());
        let mut run = request(KEYED, Some("sk-one"));
        run.host.credentials.other_models = vec![(KEYED_TOO, ProviderKey::new("sk-two"))];
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
            api.turn_blocking(&turn, &mut |_| {}, &CancellationToken::new())
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
    fn the_web_search_uses_the_run_s_account_and_needs_its_key() {
        let http = Arc::new(ScriptedHttp::new([]));
        let apis = ProviderApis::new(http.clone());
        assert!(apis.searcher(&request(KEYED, None)).is_none(), "no key, no account search");
        let searcher = apis.searcher(&request(KEYED, Some("sk-one"))).expect("searcher");
        let searched =
            platform::runtime::block_on(searcher.search("rust", &CancellationToken::new())).expect("runtime");
        assert!(searched.is_none(), "this provider has no search of its own");
        assert!(http.sent().is_empty());
    }

    #[test]
    fn a_provider_s_catalog_is_listed_once_and_only_with_its_key() {
        let page = r#"{"data":[{"id":"big"},{"id":"tiny"}]}"#.to_owned();
        let http = Arc::new(ScriptedHttp::new([(200, page)]));
        let apis = ProviderApis::new(http.clone());
        assert!(apis.catalog(&request(KEYED, None)).is_empty(), "no key, no listing");
        let ids = |listed: Vec<ModelOption>| listed.into_iter().map(|m| m.id).collect::<Vec<_>>();
        assert_eq!(ids(apis.catalog(&request(KEYED, Some("sk-one")))), ["keyed:big", "keyed:tiny"]);
        assert_eq!(ids(apis.catalog(&request(KEYED, Some("sk-one")))), ["keyed:big", "keyed:tiny"]);
        assert_eq!(http.sent().len(), 1, "the second run reads the cache");
    }

    #[test]
    fn a_signed_out_account_hands_the_turn_to_the_next_account_of_the_pool() {
        let http = Arc::new(ScriptedHttp::new([(401, "{}".to_owned()), (400, "{}".to_owned())]));
        let apis = ProviderApis::new(http.clone());
        let mut run = request(KEYED, Some("sk-one"));
        run.host.credentials.model_pool = vec![ProviderKey::new("sk-two")];
        let api = apis.api(&run).expect("api");
        let turn = TurnRequest {
            model: "model-a".into(),
            system: String::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Low,
            max_tokens: 1,
        };
        let _ = api.turn_blocking(&turn, &mut |_| {}, &CancellationToken::new());
        let keys: Vec<bool> =
            http.sent().iter().map(|sent| sent.headers.iter().any(|(_, value)| value.contains("sk-two"))).collect();
        assert_eq!(keys, [false, true], "the second account answered the same turn");
    }

    #[test]
    fn a_provider_without_a_key_says_so() {
        let apis = ProviderApis::new(Arc::new(ScriptedHttp::new([])));
        let missing = apis.api(&request(KEYED, None)).err().expect("no key");
        assert!(matches!(missing, ModelError::Unauthorized(ref why) if why.contains("Keyed")), "{missing:?}");
    }
}
