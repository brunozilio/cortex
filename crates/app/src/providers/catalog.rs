use std::sync::Arc;

use crate::api::{Reply, Unsolicited};
use agent_engine::model::http::NetHttp;
use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider};

use super::ModelProviders;
use crate::handler::outbox::Replies;

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
    pub(crate) fn discover_models(&self, engine_models: Vec<ModelOption>, replies: &Replies) {
        let (this, replies) = (self.clone(), replies.clone());
        platform::named::spawn("model-catalog", move || {
            let accounts: Vec<_> = agent_engine::providers::all()
                .into_iter()
                .map(llm_providers::id)
                .filter_map(|provider| Some((provider, this.key(provider).ok()?)))
                .collect();
            let mut models = list(this.catalog.as_ref(), accounts, &replies);
            models.extend(engine_models);
            replies.send(Reply::Models(models));
        });
    }
}

fn list(
    catalog: &dyn ModelCatalog,
    accounts: Vec<(ModelProvider, ProviderKey)>,
    replies: &Replies,
) -> Vec<ModelOption> {
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
            Err(message) => replies.tell(Unsolicited::CatalogFailed { provider, message }),
        }
    }
    models
}
