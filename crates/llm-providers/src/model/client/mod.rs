pub mod oauth;
mod transport;
mod turns;

use super::http::Http;
use super::{BoxFuture, CancellationToken, KeySource, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use domain::{ModelOption, ModelProvider};
use std::sync::Arc;
use tokio::sync::Mutex;

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
            Arc::new(Transport { provider, http, keys, turns: Default::default(), cached: Mutex::new(None), model });
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

    pub async fn search(
        &self,
        query: &str,
        cancel: &CancellationToken,
    ) -> Option<Result<Vec<llm_providers::web_search::Hit>, ModelError>> {
        self.ready().ok()?.transport.search(query, cancel).await
    }
}

impl ModelApi for ProviderClient {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            let ready = self.ready()?;
            {
                let Some(_turn) = ready.transport.turns.take_unless(cancel).await else {
                    return Err(ModelError::Cancelled);
                };
                ready.transport.config_under(false, cancel).await?;
            }
            ready.api.turn(request, on, cancel).await
        })
    }
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod refresh_tests;
