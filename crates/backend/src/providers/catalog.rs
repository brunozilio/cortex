use std::sync::Arc;

use agent_engine::model::http::NetHttp;
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider};
use platform::sync::Locked;
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
        agent_engine::model::client::ProviderClient::new(provider, Arc::new(NetHttp), Arc::new(key))
            .models()
            .map_err(|error| error.to_string())
    }
}

impl ModelProviders {
    pub(crate) fn discover_models(&self, engine_models: Vec<ModelOption>, outbox: &Outbox) {
        let generation = {
            let mut value = self.generation.locked();
            *value = value.wrapping_add(1);
            *value
        };
        let (this, outbox) = (self.clone(), outbox.clone());
        platform::named::spawn("model-catalog", move || {
            let accounts: Vec<_> = agent_engine::providers::all()
                .into_iter()
                .map(llm_providers::id)
                .filter_map(|provider| Some((provider, this.key(provider).ok()?)))
                .collect();
            let mut models = list(this.catalog.as_ref(), accounts, &outbox);
            models.extend(engine_models);
            let latest = *this.generation.locked() == generation;
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
