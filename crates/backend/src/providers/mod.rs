mod account;
mod catalog;
mod kiro_sign_in;
pub(crate) mod redirect;
mod store;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use domain::provider::{ModelProvider, ProviderAccount, ProviderKey, clean_key, key_hint};
use domain::{Principal, SecretId};
use llm_providers::Config;
use protocol::{ProviderEvent, ServerEvent};

pub use self::account::{BrowserSignIn, SignIn};
pub use self::catalog::{ModelCatalog, ProviderCatalog};
pub use self::store::{FileProviderStore, InMemoryProviderStore, ProviderStore};
use crate::outbox::Outbox;
use crate::secrets::SecretStore;

pub(crate) const fn secret_id(provider: ModelProvider) -> SecretId {
    SecretId::from_uuid(uuid::Uuid::from_u128(match provider {
        ModelProvider::OpenAi => 0x7072_6f76_6964_6572_0000_0000_0000_0001,
        ModelProvider::Anthropic => 0x7072_6f76_6964_6572_0000_0000_0000_0002,
        ModelProvider::ChatGptSubscription => 0x7072_6f76_6964_6572_0000_0000_0000_0004,
        ModelProvider::ClaudeSubscription => 0x7072_6f76_6964_6572_0000_0000_0000_0005,
        ModelProvider::CopilotSubscription => 0x7072_6f76_6964_6572_0000_0000_0000_0006,
        ModelProvider::KiroSubscription => 0x7072_6f76_6964_6572_0000_0000_0000_0007,
    }))
}

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

#[derive(Clone)]
pub struct ModelProviders {
    store: Arc<dyn ProviderStore>,
    secrets: Arc<dyn SecretStore>,
    sign_in: Arc<dyn SignIn>,
    catalog: Arc<dyn ModelCatalog>,
    generations: Arc<Mutex<HashMap<String, u64>>>,
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
        Self {
            store,
            secrets,
            sign_in: Arc::new(BrowserSignIn),
            catalog: Arc::new(ProviderCatalog),
            generations: Arc::default(),
            lock: Arc::default(),
            clock: Arc::new(system_clock),
        }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(FileProviderStore::system()), crate::secrets::system_secrets())
    }

    pub fn with_secrets(mut self, secrets: Arc<dyn SecretStore>) -> Self {
        self.secrets = secrets;
        self
    }

    pub fn with_store(mut self, store: Arc<dyn ProviderStore>) -> Self {
        self.store = store;
        self
    }

    pub fn with_sign_in(mut self, sign_in: Arc<dyn SignIn>) -> Self {
        self.sign_in = sign_in;
        self
    }

    pub fn with_catalog(mut self, catalog: Arc<dyn ModelCatalog>) -> Self {
        self.catalog = catalog;
        self
    }

    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    pub(crate) fn for_client(mut self) -> Self {
        self.generations = Arc::default();
        self
    }

    pub fn list(&self, owner: &Principal) -> ServerEvent {
        ServerEvent::Providers(ProviderEvent::Listed { providers: self.store.load(owner).summaries() })
    }

    pub fn connect(&self, owner: &Principal, provider: ModelProvider, raw: &str) -> ServerEvent {
        let result = self.parse(provider, raw).and_then(|(config, label)| self.keep(owner, provider, label, &config));
        self.answer(owner, provider, result)
    }

    fn parse(&self, provider: ModelProvider, raw: &str) -> Result<(Config, String), String> {
        if provider.subscription() {
            let config = Config::parse(provider, raw).map_err(|e| e.to_string())?;
            return Ok((config, provider.name().to_owned()));
        }
        let key = clean_key(raw).map_err(|problem| problem.to_string())?;
        let config = Config { api_key: key.clone(), ..Config::default() };
        config.validate(provider).map_err(|e| e.to_string())?;
        Ok((config, key_hint(&key)))
    }

    pub fn sign_in(&self, owner: &Principal, provider: ModelProvider, outbox: &Outbox) {
        if !provider.subscription() {
            let message = format!("{} connects with an API key", provider.name());
            return outbox.send(ServerEvent::Providers(ProviderEvent::Failed { provider, message }));
        }
        let (this, owner, outbox) = (self.clone(), owner.clone(), outbox.clone());
        std::thread::spawn(move || {
            let mut open = |url: String, code: Option<String>| {
                outbox.send(ServerEvent::Providers(ProviderEvent::AuthorizeAt { provider, url, code }));
            };
            let result = this
                .sign_in
                .sign_in(provider, &mut open)
                .and_then(|config| this.keep(&owner, provider, provider.name().to_owned(), &config));
            outbox.send(this.answer(&owner, provider, result));
        });
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
        use domain::provider::ProviderKeyStore;
        if self.store.load(owner).get(provider).is_none() {
            return Err(format!("{} is not connected", provider.name()));
        }
        let source = Arc::new(account::AccountStore {
            secrets: self.secrets.clone(),
            owner: owner.clone(),
            slot: secret_id(provider),
            provider,
        });
        Ok(ProviderKey::new(source.load()?).with_store(source))
    }

    fn now(&self) -> i64 {
        i64::try_from((self.clock)()).unwrap_or(i64::MAX)
    }

    fn keep(&self, owner: &Principal, provider: ModelProvider, label: String, config: &Config) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.secrets.set(owner, secret_id(provider), &account::secrets(config)?)?;
        let mut book = self.store.load(owner);
        book.connect(provider, ProviderAccount { label, since_unix: self.now() });
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
