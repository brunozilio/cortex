mod account;
mod accounts;
mod catalog;
mod port;
pub(crate) mod redirect;
mod shared;
mod signing;
mod store;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use domain::provider::{ModelProvider, ProviderAccount, ProviderBook, ProviderKey, clean_key, key_hint};
use domain::{Principal, SecretId};
use llm_providers::Config;
use protocol::{ProviderEvent, ServerEvent};

pub use self::account::{BrowserSignIn, SignIn};
pub use self::catalog::{ModelCatalog, ProviderCatalog};
pub use self::port::{Ports, SystemPorts};
pub use self::shared::{NothingShared, SharedCredentials};
pub use self::store::{FileProviderStore, InMemoryProviderStore, ProviderStore};
use crate::secrets::SecretStore;

const UNKNOWN_SLOT: u128 = 0x7072_6f76_6964_6572_ffff_ffff_ffff_ffff;

pub(crate) fn secret_id(provider: ModelProvider) -> SecretId {
    let slot = agent_engine::providers::find(provider).map_or(UNKNOWN_SLOT, |known| known.slot());
    SecretId::from_uuid(uuid::Uuid::from_u128(slot))
}

pub(crate) fn slot(provider: ModelProvider, account: &str) -> SecretId {
    match uuid::Uuid::parse_str(account) {
        Ok(id) if !account.is_empty() => SecretId::from_uuid(id),
        _ => secret_id(provider),
    }
}

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

#[derive(Clone)]
pub struct ModelProviders {
    store: Arc<dyn ProviderStore>,
    secrets: Arc<dyn SecretStore>,
    shared: Arc<dyn SharedCredentials>,
    sign_in: Arc<dyn SignIn>,
    ports: Arc<dyn Ports>,
    catalog: Arc<dyn ModelCatalog>,
    generations: Arc<Mutex<HashMap<String, u64>>>,
    runs: Arc<Mutex<HashMap<String, u64>>>,
    lock: Arc<Mutex<()>>,
    in_flight: Arc<Mutex<HashMap<ModelProvider, Arc<signing::InFlight>>>>,
    told_unkept: Arc<AtomicBool>,
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
            shared: Arc::new(NothingShared),
            sign_in: Arc::new(BrowserSignIn),
            ports: Arc::new(SystemPorts),
            catalog: Arc::new(ProviderCatalog),
            generations: Arc::default(),
            runs: Arc::default(),
            lock: Arc::default(),
            in_flight: Arc::default(),
            told_unkept: Arc::default(),
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

    pub fn with_shared(mut self, shared: Arc<dyn SharedCredentials>) -> Self {
        self.shared = shared;
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

    pub fn with_ports(mut self, ports: Arc<dyn Ports>) -> Self {
        self.ports = ports;
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
        ServerEvent::Providers(ProviderEvent::Listed {
            providers: self.book(owner).summaries(agent_engine::providers::infos()),
        })
    }

    pub fn connect(&self, owner: &Principal, provider: ModelProvider, raw: &str) -> ServerEvent {
        let kept = self.secrets.unkept().map_or(Ok(()), |reason| Err(crate::secrets::refusal(&reason)));
        let result = kept
            .and_then(|()| self.parse(provider, raw))
            .and_then(|(config, label)| self.keep(owner, provider, label, &config));
        self.answer(owner, provider, result)
    }

    pub fn unkept_notice(&self) -> Option<domain::inbox::Notice> {
        let reason = self.secrets.unkept()?;
        let told = self.told_unkept.swap(true, Ordering::SeqCst);
        (!told).then(|| {
            let text = format!(
                "{}. Connecting a provider or signing in fails until this is fixed.",
                crate::secrets::refusal(&reason)
            );
            domain::inbox::Notice::new("secrets", text)
        })
    }

    fn parse(&self, provider: ModelProvider, raw: &str) -> Result<(Config, String), String> {
        let provider = agent_engine::providers::get(provider).map_err(|e| e.to_string())?;
        if provider.access().subscription() {
            let config = Config::parse(provider, raw).map_err(|e| e.to_string())?;
            return Ok((config, provider.name().to_owned()));
        }
        let key = clean_key(raw).map_err(|problem| problem.to_string())?;
        let config = Config { api_key: key.clone(), ..Config::default() };
        config.validate(provider).map_err(|e| e.to_string())?;
        Ok((config, key_hint(&key)))
    }

    pub fn key(&self, owner: &Principal, provider: ModelProvider) -> Result<ProviderKey, String> {
        let Some(holder) = self.holder(owner, provider) else {
            return Err(format!("{} is not connected", agent_engine::providers::name(provider)));
        };
        let id = self.store.load(&holder).get(provider).map(|account| account.id.clone()).unwrap_or_default();
        self.account_key(holder, provider, &id)
    }

    fn account_key(&self, holder: Principal, provider: ModelProvider, id: &str) -> Result<ProviderKey, String> {
        use domain::provider::ProviderKeyStore;
        let source = Arc::new(account::AccountStore {
            secrets: self.secrets.clone(),
            owner: holder,
            slot: slot(provider, id),
            provider,
        });
        Ok(ProviderKey::new(source.load()?).with_store(source))
    }

    fn shared_holder(&self, owner: &Principal) -> Option<Principal> {
        self.shared.holder(owner).filter(|holder| holder != owner)
    }

    fn book(&self, owner: &Principal) -> ProviderBook {
        let own = self.store.load(owner);
        match self.shared_holder(owner) {
            Some(holder) => own.over(&self.store.load(&holder)),
            None => own,
        }
    }

    fn holder(&self, owner: &Principal, provider: ModelProvider) -> Option<Principal> {
        if self.store.load(owner).get(provider).is_some() {
            return Some(owner.clone());
        }
        self.shared_holder(owner).filter(|holder| self.store.load(holder).get(provider).is_some())
    }

    fn now(&self) -> i64 {
        i64::try_from((self.clock)()).unwrap_or(i64::MAX)
    }

    fn keep(&self, owner: &Principal, provider: ModelProvider, label: String, config: &Config) -> Result<(), String> {
        let _guard = self.lock.locked();
        let label = accounts::identity(config, &agent_engine::providers::name(provider))
            .unwrap_or_else(|| accounts::unnamed(config, label, self.store.load(owner).accounts(provider).len()));
        let id = self.store.load(owner).slot_for(provider, &label, || uuid::Uuid::new_v4().to_string());
        self.secrets.set(owner, slot(provider, &id), &account::secrets(config)?)?;
        let account = ProviderAccount { label, since_unix: self.now(), id };
        self.store
            .update(owner, &mut |book| {
                book.connect(provider, account.clone());
            })
            .map_err(|error| error.to_string())
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
