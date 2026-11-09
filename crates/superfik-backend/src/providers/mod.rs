//! Model providers on the backend: each owner's API key with OpenAI and
//! Anthropic (ADR 0015). The keys stay in the [`SecretStore`]; the client
//! only sees [`ProviderSummary`](superfik_core::provider::ProviderSummary)s.
//! The native engine asks [`ModelProviders::key`].

mod store;

use std::sync::{Arc, Mutex, PoisonError};

use superfik_core::provider::{KeyProblem, ModelProvider, ProviderAccount, ProviderKey, clean_key, key_hint};
use superfik_core::{ConnectionId, Principal};
use superfik_protocol::{ProviderEvent, ServerEvent};

pub use self::store::{FileProviderStore, InMemoryProviderStore, ProviderStore};
use crate::plugins::{KeychainSecretStore, SecretStore, Secrets};

const API_KEY: &str = "api_key";

pub(crate) const fn secret_id(provider: ModelProvider) -> ConnectionId {
    ConnectionId::from_uuid(uuid::Uuid::from_u128(match provider {
        ModelProvider::OpenAi => 0x7072_6f76_6964_6572_0000_0000_0000_0001,
        ModelProvider::Anthropic => 0x7072_6f76_6964_6572_0000_0000_0000_0002,
        ModelProvider::ChatGptSubscription => 0x7072_6f76_6964_6572_0000_0000_0000_0004,
        ModelProvider::ClaudeSubscription => 0x7072_6f76_6964_6572_0000_0000_0000_0005,
        ModelProvider::Local => 0x7072_6f76_6964_6572_0000_0000_0000_0003,
    }))
}

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

#[derive(Clone)]
pub struct ModelProviders {
    store: Arc<dyn ProviderStore>,
    secrets: Arc<dyn SecretStore>,
    lock: Arc<Mutex<()>>,
    clock: Clock,
}

impl std::fmt::Debug for ModelProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelProviders").finish_non_exhaustive()
    }
}

fn system_clock() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl ModelProviders {
    pub fn new(store: Arc<dyn ProviderStore>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { store, secrets, lock: Arc::default(), clock: Arc::new(system_clock) }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(FileProviderStore::system()), Arc::new(KeychainSecretStore))
    }

    pub fn with_secrets(mut self, secrets: Arc<dyn SecretStore>) -> Self {
        self.secrets = secrets;
        self
    }

    pub fn with_store(mut self, store: Arc<dyn ProviderStore>) -> Self {
        self.store = store;
        self
    }

    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    pub fn list(&self, owner: &Principal) -> ServerEvent {
        ServerEvent::Providers(ProviderEvent::Listed { providers: self.store.load(owner).summaries() })
    }

    pub fn connect(&self, owner: &Principal, provider: ModelProvider, raw: &str) -> ServerEvent {
        if provider.subscription() {
            let result = superfik_plugins::providers::Config::parse(provider, raw).map_err(|e| e.to_string()).and_then(
                |config| {
                    let account = ProviderAccount { label: provider.name().to_owned(), since_unix: self.now() };
                    let raw = serde_json::to_string(&config).map_err(|e| e.to_string())?;
                    self.keep(owner, provider, account, &Secrets::from([("config".to_owned(), raw)]))
                },
            );
            return self.answer(owner, provider, result);
        }
        let result = Self::accepts(provider).and_then(|()| clean_key(raw)).map_err(|p| p.to_string()).and_then(|key| {
            let account = ProviderAccount { label: key_hint(&key), since_unix: self.now() };
            self.keep(owner, provider, account, &Secrets::from([(API_KEY.to_owned(), key)]))
        });
        self.answer(owner, provider, result)
    }

    const fn accepts(provider: ModelProvider) -> Result<(), KeyProblem> {
        if provider.needs_account() { Ok(()) } else { Err(KeyProblem::NoAccount) }
    }

    pub fn disconnect(&self, owner: &Principal, provider: ModelProvider) -> ServerEvent {
        let result = {
            let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
            let mut book = self.store.load(owner);
            book.disconnect(provider);
            self.secrets.delete(owner, secret_id(provider));
            self.store.save(owner, &book).map_err(|error| error.to_string())
        };
        self.answer(owner, provider, result)
    }

    pub fn key(&self, owner: &Principal, provider: ModelProvider) -> Result<ProviderKey, String> {
        if self.store.load(owner).get(provider).is_none() {
            return Err(format!("{} is not connected: connect it in Settings", provider.name()));
        }
        if provider.subscription() {
            use superfik_core::provider::ProviderKeyStore;
            let source = Arc::new(crate::plugins::model_providers::AccountStore {
                secrets: self.secrets.clone(),
                owner: owner.clone(),
                connection: secret_id(provider),
                provider,
            });
            return Ok(ProviderKey::new(source.load()?).with_store(source));
        }
        let secrets = self.secrets.get(owner, secret_id(provider));
        secrets
            .and_then(|secrets| secrets.get(API_KEY).cloned())
            .map(ProviderKey::new)
            .ok_or_else(|| format!("{} is not connected: add its API key in Settings", provider.name()))
    }

    fn now(&self) -> i64 {
        i64::try_from((self.clock)()).unwrap_or(i64::MAX)
    }

    fn keep(
        &self,
        owner: &Principal,
        provider: ModelProvider,
        account: ProviderAccount,
        secrets: &Secrets,
    ) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.secrets.set(owner, secret_id(provider), secrets)?;
        let mut book = self.store.load(owner);
        book.connect(provider, account);
        self.store.save(owner, &book).map_err(|error| error.to_string())
    }

    fn answer(&self, owner: &Principal, provider: ModelProvider, result: Result<(), String>) -> ServerEvent {
        match result {
            Ok(()) => self.list(owner),
            Err(message) => {
                tracing::warn!(%message, ?provider, "model provider command failed");
                ServerEvent::Providers(ProviderEvent::Failed { provider, message })
            }
        }
    }
}

#[cfg(test)]
mod tests;
