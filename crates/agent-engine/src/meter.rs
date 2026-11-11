use crate::RunGate;
use crate::model::{Delta, ModelApi, ModelError, Reply, TurnRequest};
use domain::billing::{ModelUsage, PRICE_DATE};
use llm_providers::rates;
use domain::{EngineEvent, ModelProvider};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct Ledger {
    costs: Mutex<Vec<Option<f64>>>,
}
impl Ledger {
    pub fn cost(&self) -> Option<f64> {
        self.costs.lock().ok()?.iter().copied().sum()
    }
}
pub type Calls = Arc<dyn Fn(&TurnRequest, &Reply) + Send + Sync>;

pub struct Metered {
    pub api: Arc<dyn ModelApi>,
    pub ledger: Arc<Ledger>,
    pub provider: ModelProvider,
    pub gate: RunGate,
    pub model: domain::ModelOption,
    pub calls: Option<Calls>,
}
impl ModelApi for Metered {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let reply = self.api.turn(request, on, stop)?;
        if let Some(calls) = &self.calls {
            calls(request, &reply);
        }
        let u = reply.usage;
        let provider = llm_providers::find(self.provider);
        let vendor = provider.map_or("", |provider| provider.vendor());
        let cost = rates(vendor, &request.model, u.prompt())
            .map(|rate| rate.estimate(u.input, u.output, u.cache_read, u.cache_write));
        self.ledger.costs.lock().map_err(|_| ModelError::Provider("usage ledger unavailable".into()))?.push(cost);
        self.gate.emit(EngineEvent::Usage(ModelUsage {
            model: request.model.clone(),
            input: u.input,
            output: u.output,
            cache_read: u.cache_read,
            cache_write: u.cache_write,
            estimated_cost_usd: cost,
            basis: if provider.is_some_and(|provider| provider.access().subscription()) {
                "subscription-api-equivalent"
            } else {
                "api-reference-estimate"
            }
            .into(),
            price_date: PRICE_DATE.into(),
        }));
        Ok(reply)
    }
}
impl std::fmt::Debug for Metered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metered").field("provider", &self.provider).finish_non_exhaustive()
    }
}
