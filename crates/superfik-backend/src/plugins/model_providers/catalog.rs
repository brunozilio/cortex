//! Listing what the connected AI accounts may run: each account's catalog,
//! asked all at once off the command loop, published as the model picker.

use std::sync::Arc;

use superfik_core::ModelProvider;
use superfik_core::provider::ProviderKey;
use superfik_native::model::http::UreqHttp;

use super::Account;
use crate::plugins::Plugins;

impl Plugins {
    pub(crate) fn discover_models(
        &self,
        owner: &superfik_core::Principal,
        providers: &crate::providers::ModelProviders,
        local_models: Vec<superfik_core::ModelOption>,
        engines: Vec<superfik_core::EngineInfo>,
        outbox: &crate::outbox::Outbox,
    ) {
        let owner_key = crate::memory::owner_key(owner);
        let generation = {
            let mut generations = self.model_generations.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let value = generations.entry(owner_key.clone()).or_default();
            *value = value.wrapping_add(1);
            *value
        };
        let (plugins, providers, owner) = (self.clone(), providers.clone(), owner.clone());
        let catalog = self.model_catalog.clone();
        let generations = self.model_generations.clone();
        let outbox = outbox.clone();
        std::thread::spawn(move || {
            let accounts = plugins.ai_accounts(&owner, &providers);
            let mut models = list_accounts(catalog.as_ref(), accounts, &outbox);
            models.extend(local_models);
            if generations.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(&owner_key)
                == Some(&generation)
            {
                publish_models(models, engines, &outbox);
            }
        });
    }
}

pub trait ModelCatalog: Send + Sync {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<superfik_core::ModelOption>, String>;
}

pub struct ProviderCatalog;

impl ModelCatalog for ProviderCatalog {
    fn models(&self, provider: ModelProvider, key: ProviderKey) -> Result<Vec<superfik_core::ModelOption>, String> {
        superfik_native::model::providers::ProviderClient::new(provider, Arc::new(UreqHttp), Arc::new(key))
            .models()
            .map_err(|error| error.to_string())
    }
}

fn list_accounts(
    catalog: &dyn ModelCatalog,
    accounts: Vec<Account>,
    outbox: &crate::outbox::Outbox,
) -> Vec<superfik_core::ModelOption> {
    let listed: Vec<_> = std::thread::scope(|scope| {
        let asks: Vec<_> = accounts
            .into_iter()
            .map(|(provider, account, key)| (provider, account, scope.spawn(move || catalog.models(provider, key))))
            .collect();
        asks.into_iter()
            .map(|(provider, account, ask)| {
                (provider, account, ask.join().unwrap_or_else(|_| Err("model catalog panicked".to_owned())))
            })
            .collect()
    });
    let mut models = Vec::new();
    for (provider, account, listed) in listed {
        match listed {
            Ok(listed) => models.extend(listed.into_iter().map(|model| {
                let model = model.served_by(provider);
                match account {
                    Some(account) => model.of_account(account),
                    None => model,
                }
            })),
            Err(message) => {
                outbox.send(superfik_protocol::ServerEvent::Providers(superfik_protocol::ProviderEvent::Failed {
                    provider,
                    message,
                }))
            }
        }
    }
    models
}

fn publish_models(
    models: Vec<superfik_core::ModelOption>,
    mut engines: Vec<superfik_core::EngineInfo>,
    outbox: &crate::outbox::Outbox,
) {
    for engine in &mut engines {
        engine.models = models.iter().filter(|model| model.engine == engine.kind).cloned().collect();
    }
    outbox.send(superfik_protocol::ServerEvent::Models(models));
    outbox.send(superfik_protocol::ServerEvent::Engines(engines));
}
