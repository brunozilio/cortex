//! Listing what the connected providers may run: each provider's catalog,
//! asked all at once off the command loop, published as the model list.

use std::sync::Arc;

use agent_engine::model::http::NetHttp;
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider, Principal};
use protocol::{ProviderEvent, ServerEvent};

use super::ModelProviders;
use crate::outbox::Outbox;

pub trait ModelCatalog: Send + Sync {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<ModelOption>, String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderCatalog;

impl ModelCatalog for ProviderCatalog {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<ModelOption>, String> {
        agent_engine::model::providers::ProviderClient::new(provider, Arc::new(NetHttp), Arc::new(key))
            .models()
            .map_err(|error| error.to_string())
    }
}

impl ModelProviders {
    /// Publishes [`ServerEvent::Models`] from a background thread; only the
    /// latest ask per owner is published.
    pub(crate) fn discover_models(&self, owner: &Principal, local_models: Vec<ModelOption>, outbox: &Outbox) {
        let owner_key = crate::memory::owner_key(owner);
        let generation = {
            let mut generations = self.generations.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let value = generations.entry(owner_key.clone()).or_default();
            *value = value.wrapping_add(1);
            *value
        };
        let (this, owner, outbox) = (self.clone(), owner.clone(), outbox.clone());
        std::thread::spawn(move || {
            let accounts: Vec<_> = ModelProvider::ACCOUNTS
                .into_iter()
                .filter_map(|provider| Some((provider, this.key(&owner, provider).ok()?)))
                .collect();
            let mut models = list(this.catalog.as_ref(), accounts, &outbox);
            models.extend(local_models);
            let latest = this.generations.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&owner_key)
                == Some(&generation);
            if latest {
                outbox.send(ServerEvent::Models(models));
            }
        });
    }
}

fn list(catalog: &dyn ModelCatalog, accounts: Vec<(ModelProvider, ProviderKey)>, outbox: &Outbox) -> Vec<ModelOption> {
    let listed: Vec<_> = std::thread::scope(|scope| {
        let asks: Vec<_> = accounts
            .into_iter()
            .map(|(provider, key)| (provider, scope.spawn(move || catalog.models(provider, key))))
            .collect();
        asks.into_iter()
            .map(|(provider, ask)| (provider, ask.join().unwrap_or_else(|_| Err("model catalog panicked".to_owned()))))
            .collect()
    });
    let mut models = Vec::new();
    for (provider, listed) in listed {
        match listed {
            Ok(listed) => models.extend(listed.into_iter().map(|model| model.served_by(provider))),
            Err(message) => outbox.send(ServerEvent::Providers(ProviderEvent::Failed { provider, message })),
        }
    }
    models
}
