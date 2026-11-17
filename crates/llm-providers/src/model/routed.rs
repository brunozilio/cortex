use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use platform::sync::Locked;

use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider};

use super::client::ProviderClient;
use super::http::Http;
use super::{BoxFuture, CancellationToken, ModelApi, ModelError, OnDelta, Reply, TurnRequest};

pub struct Routed {
    main: Arc<dyn ModelApi>,
    provider: ModelProvider,
    http: Arc<dyn Http>,
    others: Vec<(ModelProvider, ProviderKey)>,
    built: Mutex<HashMap<(ModelProvider, String), Arc<dyn ModelApi>>>,
}

impl std::fmt::Debug for Routed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let others: Vec<ModelProvider> = self.others.iter().map(|(provider, _)| *provider).collect();
        f.debug_struct("Routed").field("provider", &self.provider).field("others", &others).finish_non_exhaustive()
    }
}

impl Routed {
    pub fn new(
        main: Arc<dyn ModelApi>,
        provider: ModelProvider,
        http: Arc<dyn Http>,
        others: Vec<(ModelProvider, ProviderKey)>,
    ) -> Self {
        Self { main, provider, http, others, built: Mutex::default() }
    }

    fn other(&self, provider: ModelProvider, model: &str) -> Result<Arc<dyn ModelApi>, ModelError> {
        let mut built = self.built.locked();
        if let Some(api) = built.get(&(provider, model.to_owned())) {
            return Ok(Arc::clone(api));
        }
        let Some((_, key)) = self.others.iter().find(|(connected, _)| *connected == provider) else {
            return Err(ModelError::Unauthorized(format!(
                "{} is not connected: run `{} login`",
                crate::providers::name(provider),
                brand::COMMAND
            )));
        };
        let option = ModelOption::new(model, model).served_by(provider);
        let api: Arc<dyn ModelApi> =
            Arc::new(ProviderClient::for_model(option, Arc::clone(&self.http), Arc::new(key.clone())));
        built.insert((provider, model.to_owned()), Arc::clone(&api));
        Ok(api)
    }
}

impl ModelApi for Routed {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            let known: Vec<&str> = crate::providers::all().iter().map(|provider| provider.id()).collect();
            let route = domain::routing::route(&request.model, &known);
            match route.provider.map(ModelProvider::named) {
                Some(provider) if provider != self.provider => {
                    self.other(provider, &request.model)?.turn(request, on, cancel).await
                }
                _ => self.main.turn(request, on, cancel).await,
            }
        })
    }
}
