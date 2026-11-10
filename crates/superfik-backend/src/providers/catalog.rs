//! Listing what the connected providers may run: each provider's catalog,
//! asked all at once off the command loop, published as the model list.

use std::sync::Arc;

use superfik_core::provider::ProviderKey;
use superfik_core::{EngineInfo, ModelOption, ModelProvider, Principal};
use superfik_native::model::http::UreqHttp;
use superfik_protocol::{ProviderEvent, ServerEvent};

use super::ModelProviders;
use crate::outbox::Outbox;

pub trait ModelCatalog: Send + Sync {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<ModelOption>, String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderCatalog;

impl ModelCatalog for ProviderCatalog {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<ModelOption>, String> {
        superfik_native::model::providers::ProviderClient::new(provider, Arc::new(UreqHttp), Arc::new(key))
            .models()
            .map_err(|error| error.to_string())
    }
}

impl ModelProviders {
    /// Publishes [`ServerEvent::Models`] and [`ServerEvent::Engines`] from
    /// a background thread; only the latest ask per owner is published.
    pub(crate) fn discover_models(
        &self,
        owner: &Principal,
        local_models: Vec<ModelOption>,
        engines: Vec<EngineInfo>,
        outbox: &Outbox,
    ) {
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
                publish(models, engines, &outbox);
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

fn publish(models: Vec<ModelOption>, mut engines: Vec<EngineInfo>, outbox: &Outbox) {
    for engine in &mut engines {
        engine.models = models.iter().filter(|model| model.engine == engine.kind).cloned().collect();
    }
    outbox.send(ServerEvent::Models(models));
    outbox.send(ServerEvent::Engines(engines));
}
