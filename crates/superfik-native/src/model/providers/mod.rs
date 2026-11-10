//! The provider plugins' accounts as model clients. What each plugin sends
//! and expects is in `superfik_plugins::providers`; this module does the IO.

pub mod oauth;
mod transport;

use super::http::Http;
use super::{Delta, KeySource, ModelApi, ModelError, Reply, TurnRequest};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use superfik_core::{ModelOption, ModelProvider};

pub use superfik_plugins::providers::Config;
pub use superfik_plugins::providers::oauth::Tokens;
use transport::Transport;

pub struct ProviderClient {
    transport: Arc<Transport>,
    api: Arc<dyn ModelApi>,
}

impl std::fmt::Debug for ProviderClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderClient").field("provider", &self.transport.provider).finish_non_exhaustive()
    }
}

impl ProviderClient {
    pub fn new(provider: ModelProvider, http: Arc<dyn Http>, keys: Arc<dyn KeySource>) -> Self {
        Self::configured(provider, http, keys, None)
    }

    pub fn for_model(model: ModelOption, http: Arc<dyn Http>, keys: Arc<dyn KeySource>) -> Self {
        Self::configured(model.provider, http, keys, Some(model))
    }

    fn configured(
        provider: ModelProvider,
        http: Arc<dyn Http>,
        keys: Arc<dyn KeySource>,
        model: Option<ModelOption>,
    ) -> Self {
        let transport =
            Arc::new(Transport { provider, http, keys, lock: Mutex::new(()), cached: Mutex::new(None), model });
        let dummy = Arc::new(superfik_core::provider::ProviderKey::new("transport-owned"));
        let api: Arc<dyn ModelApi> = match provider {
            ModelProvider::OpenAi | ModelProvider::ChatGptSubscription => {
                Arc::new(super::openai::Responses::new(transport.clone(), dummy))
            }
            _ => Arc::new(super::anthropic::Messages::new(transport.clone(), dummy)),
        };
        Self { transport, api }
    }

    pub fn models(&self) -> Result<Vec<ModelOption>, ModelError> {
        self.transport.models()
    }
}

impl ModelApi for ProviderClient {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        {
            let _guard = self.transport.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            self.transport.config(false)?;
        }
        self.api.turn(request, on, stop)
    }
}
