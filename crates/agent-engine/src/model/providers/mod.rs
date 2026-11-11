pub mod oauth;
mod transport;

use super::http::Http;
use super::{Delta, KeySource, ModelApi, ModelError, Reply, TurnRequest};
use domain::{ModelOption, ModelProvider};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub use llm_providers::Config;
use llm_providers::Wire;
pub use llm_providers::oauth::Tokens;
use transport::Transport;

struct SameModel;

impl super::chat::Endpoints for SameModel {
    fn endpoint(&self, model: &str) -> Result<super::chat::Endpoint, ModelError> {
        Ok(super::chat::Endpoint { base_url: String::new(), model: model.to_owned() })
    }
}

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
        let provider = llm_providers::get(provider)?;
        let transport =
            Arc::new(Transport { provider, http, keys, lock: Mutex::new(()), cached: Mutex::new(None), model });
        let dummy = Arc::new(domain::provider::ProviderKey::new("transport-owned"));
        let api: Arc<dyn ModelApi> = match provider.wire(transport.model.as_ref()) {
            Wire::Responses => Arc::new(super::openai::Responses::new(transport.clone(), dummy)),
            Wire::Messages => Arc::new(super::anthropic::Messages::new(transport.clone(), dummy)),
            Wire::Chat => Arc::new(super::chat::Chat::new(transport.clone(), Arc::new(SameModel))),
            Wire::EventStream => {
                let window = transport.model.as_ref().and_then(|model| model.window);
                Arc::new(super::event_stream::Conversation::new(transport.clone(), window))
            }
        };
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
