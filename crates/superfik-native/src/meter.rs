//! Every successful provider call is metered, including compaction and children.
use crate::model::{Delta, ModelApi, ModelError, Reply, TurnRequest};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use superfik_core::billing::{ModelUsage, PRICE_DATE, rates};
use superfik_core::{EngineEvent, ModelProvider};
use superfik_engine::RunGate;

#[derive(Debug, Default)]
pub struct Ledger {
    costs: Mutex<Vec<Option<f64>>>,
}
impl Ledger {
    pub fn cost(&self) -> Option<f64> {
        self.costs.lock().ok()?.iter().copied().sum()
    }
}
pub struct Metered {
    pub api: Arc<dyn ModelApi>,
    pub ledger: Arc<Ledger>,
    pub provider: ModelProvider,
    pub gate: RunGate,
    pub model: superfik_core::ModelOption,
}
impl ModelApi for Metered {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let reply = self.api.turn(request, on, stop)?;
        let u = reply.usage;
        let cost = rates(self.provider, &request.model, u.prompt())
            .map(|rate| rate.estimate(u.input, u.output, u.cache_read, u.cache_write));
        self.ledger.costs.lock().map_err(|_| ModelError::Provider("usage ledger unavailable".into()))?.push(cost);
        self.gate.emit(EngineEvent::Usage(ModelUsage {
            model: request.model.clone(),
            input: u.input,
            output: u.output,
            cache_read: u.cache_read,
            cache_write: u.cache_write,
            estimated_cost_usd: cost,
            basis: if self.provider.subscription() {
                "subscription-api-equivalent"
            } else if self.provider == ModelProvider::Local {
                "local"
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
