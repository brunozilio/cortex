use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError};

use domain::provider::ProviderKey;
use domain::{ModelOption, ModelProvider};

use super::http::Http;
use super::providers::ProviderClient;
use super::{Delta, ModelApi, ModelError, Reply, TurnRequest};

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
        let mut built = self.built.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(api) = built.get(&(provider, model.to_owned())) {
            return Ok(Arc::clone(api));
        }
        let Some((_, key)) = self.others.iter().find(|(connected, _)| *connected == provider) else {
            return Err(ModelError::Unauthorized(format!(
                "{} is not connected: run `{} login`",
                llm_providers::name(provider),
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
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let known: Vec<&str> = llm_providers::all().iter().map(|provider| provider.id()).collect();
        let route = domain::routing::route(&request.model, &known);
        match route.provider.map(ModelProvider::named) {
            Some(provider) if provider != self.provider => {
                self.other(provider, &request.model)?.turn(request, on, stop)
            }
            _ => self.main.turn(request, on, stop),
        }
    }
}
