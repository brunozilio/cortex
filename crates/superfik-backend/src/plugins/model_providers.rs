//! Model provider plugins' accounts. Keys and tokens stay in the
//! SecretStore; rotating credentials are persisted.
use super::{Plugins, SecretStore, Secrets};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::sync::Arc;
use superfik_core::provider::{ProviderKey, ProviderKeyStore};
use superfik_core::{ConnectionId, ModelProvider, Principal};
use superfik_native::model::http::UreqHttp;
use superfik_native::model::providers::oauth;
use superfik_plugins::providers::Config;

mod catalog;

pub use catalog::{ModelCatalog, ProviderCatalog};
pub use superfik_plugins::providers::oauth::subscription_client;

pub fn sign_in(provider: ModelProvider, open: &mut dyn FnMut(String)) -> Result<Secrets, String> {
    let (port, path) = superfik_plugins::providers::oauth::callback(provider);
    let listener = superfik_oauth::Loopback::bind_on(port, path, true)?;
    let redirect = listener.redirect_uri();
    let random = |length| -> Result<String, String> {
        let mut bytes = vec![0; length];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    };
    let verifier = random(64)?;
    let state = random(superfik_plugins::providers::oauth::state_bytes(provider))?;
    open(oauth::authorize(provider, &superfik_oauth::pkce::challenge(&verifier), &state, &redirect));
    let code = listener.wait(&state)?;
    let tokens =
        oauth::exchange(&UreqHttp, provider, &code, &state, &verifier, &redirect).map_err(|e| e.to_string())?;
    let config = Config { auth: Some(tokens), ..Config::default() };
    config.validate(provider).map_err(|e| e.to_string())?;
    Ok(Secrets::from([("config".into(), serde_json::to_string(&config).map_err(|e| e.to_string())?)]))
}

pub(crate) struct AccountStore {
    pub secrets: Arc<dyn SecretStore>,
    pub owner: Principal,
    pub connection: ConnectionId,
    pub provider: ModelProvider,
}

impl ProviderKeyStore for AccountStore {
    fn load(&self) -> Result<String, String> {
        let values = self.secrets.get(&self.owner, self.connection).ok_or("provider account disconnected")?;
        if let Some(config) = values.get("config") {
            return Ok(config.clone());
        }
        if let Some(auth) = values.get("auth") {
            let config = Config::parse(self.provider, auth).map_err(|e| e.to_string())?;
            return serde_json::to_string(&config).map_err(|e| e.to_string());
        }
        let config = Config {
            api_key: values.get("api_key").cloned().unwrap_or_default(),
            base_url: values.get("base_url").cloned().unwrap_or_default(),
            ..Config::default()
        };
        config.validate(self.provider).map_err(|e| e.to_string())?;
        serde_json::to_string(&config).map_err(|e| e.to_string())
    }

    fn save(&self, value: &str) -> Result<(), String> {
        Config::parse(self.provider, value).map_err(|e| e.to_string())?;
        let mut secrets = self.secrets.get(&self.owner, self.connection).ok_or("provider account disconnected")?;
        secrets.insert("config".into(), value.to_owned());
        self.secrets.set(&self.owner, self.connection, &secrets)
    }
}

impl Plugins {
    fn ai_accounts(&self, owner: &Principal, providers: &crate::providers::ModelProviders) -> Vec<Account> {
        let book = self.store.load(owner);
        ModelProvider::ACCOUNTS
            .into_iter()
            .flat_map(|provider| {
                let mut keys: Vec<Account> = book
                    .connections
                    .iter()
                    .filter(|c| c.plugin.as_str() == provider.plugin_id())
                    .filter_map(|c| Some((provider, Some(c.id), self.connection_key(owner, provider, c.id)?)))
                    .collect();
                if keys.is_empty()
                    && let Ok(key) = providers.key(owner, provider)
                {
                    keys.push((provider, None, key));
                }
                keys
            })
            .collect()
    }

    pub fn run_key(
        &self,
        owner: &Principal,
        model: &superfik_core::ModelOption,
        selected: &[superfik_core::Connection],
    ) -> Result<Option<ProviderKey>, String> {
        match model.account {
            Some(account) if model.provider.needs_account() => self
                .connection_key(owner, model.provider, account)
                .map(Some)
                .ok_or_else(|| "The model's AI account is no longer connected. Choose another model.".to_owned()),
            _ => self.model_key_for(owner, model.provider, selected),
        }
    }

    pub fn model_key_for(
        &self,
        owner: &Principal,
        provider: ModelProvider,
        selected: &[superfik_core::Connection],
    ) -> Result<Option<ProviderKey>, String> {
        if !provider.needs_account() {
            return Ok(None);
        }
        if let Some(connection) = selected.iter().find(|c| c.plugin.as_str() == provider.plugin_id()) {
            return self
                .connection_key(owner, provider, connection.id)
                .map(Some)
                .ok_or_else(|| "The selected AI account is no longer connected. Choose another account.".to_owned());
        }
        Ok(self.model_key(owner, provider))
    }

    pub fn model_key(&self, owner: &Principal, provider: ModelProvider) -> Option<ProviderKey> {
        if !provider.needs_account() {
            return None;
        }
        let book = self.store.load(owner);
        let connection = book.connections.iter().find(|c| c.plugin.as_str() == provider.plugin_id())?;
        self.connection_key(owner, provider, connection.id)
    }

    fn connection_key(
        &self,
        owner: &Principal,
        provider: ModelProvider,
        connection: ConnectionId,
    ) -> Option<ProviderKey> {
        if !self
            .store
            .load(owner)
            .connections
            .iter()
            .any(|c| c.id == connection && c.plugin.as_str() == provider.plugin_id())
        {
            return None;
        }
        let source =
            Arc::new(AccountStore { secrets: self.secrets.clone(), owner: owner.clone(), connection, provider });
        let raw = source.load().unwrap_or_default();
        Some(ProviderKey::new(raw).with_store(source))
    }
}

type Account = (ModelProvider, Option<ConnectionId>, ProviderKey);

#[cfg(test)]
mod tests;
