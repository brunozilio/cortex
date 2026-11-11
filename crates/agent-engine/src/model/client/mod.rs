pub mod oauth;
mod transport;

use super::http::Http;
use super::{Delta, KeySource, ModelApi, ModelError, Reply, TurnRequest};
use domain::{ModelOption, ModelProvider};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub use llm_providers::Config;
pub use llm_providers::oauth::Tokens;
use transport::Transport;

struct Ready {
    transport: Arc<Transport>,
    api: Arc<dyn ModelApi>,
}

pub struct ProviderClient {
    ready: Result<Ready, ModelError>,
}

impl std::fmt::Debug for ProviderClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let provider = self.ready.as_ref().map(|ready| ready.transport.provider.id());
        f.debug_struct("ProviderClient").field("provider", &provider).finish_non_exhaustive()
    }
}

impl ProviderClient {
    pub fn new(provider: ModelProvider, http: Arc<dyn Http>, keys: Arc<dyn KeySource>) -> Self {
        Self { ready: Self::configured(provider, http, keys, None) }
    }

    pub fn for_model(model: ModelOption, http: Arc<dyn Http>, keys: Arc<dyn KeySource>) -> Self {
        Self { ready: Self::configured(model.provider, http, keys, Some(model)) }
    }

    fn configured(
        provider: ModelProvider,
        http: Arc<dyn Http>,
        keys: Arc<dyn KeySource>,
        model: Option<ModelOption>,
    ) -> Result<Ready, ModelError> {
        let provider = crate::providers::get(provider)?;
        let transport =
            Arc::new(Transport { provider, http, keys, lock: Mutex::new(()), cached: Mutex::new(None), model });
        let wire = provider.wire(transport.model.as_ref());
        let api = super::wires::open(wire, transport.clone(), transport.model.as_ref()).ok_or_else(|| {
            ModelError::Provider(format!("{} speaks {wire}, which this build does not have", provider.name()))
        })?;
        Ok(Ready { transport, api })
    }

    fn ready(&self) -> Result<&Ready, ModelError> {
        self.ready.as_ref().map_err(Clone::clone)
    }

    pub fn models(&self) -> Result<Vec<ModelOption>, ModelError> {
        self.ready()?.transport.models()
    }
}

impl ModelApi for ProviderClient {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ModelError::Cancelled);
        }
        let ready = self.ready()?;
        {
            let _guard = ready.transport.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            ready.transport.config(false)?;
        }
        ready.api.turn(request, on, stop)
    }
}
