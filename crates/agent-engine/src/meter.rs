use crate::RunGate;
use crate::model::{BoxFuture, CancellationToken, ModelApi, ModelError, OnDelta, Reply, TurnRequest};
use domain::billing::{ModelUsage, PRICE_DATE};
use domain::{EngineEvent, ModelProvider};
use llm_providers::rates;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct Ledger {
    costs: Mutex<Vec<Option<f64>>>,
}
impl Ledger {
    pub fn cost(&self) -> Option<f64> {
        self.costs.lock().ok()?.iter().copied().sum()
    }

    pub fn known_cost(&self) -> f64 {
        self.costs.lock().map(|costs| costs.iter().flatten().sum()).unwrap_or(0.0)
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
    pub budget: Option<domain::budget::Budget>,
}
impl ModelApi for Metered {
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(async move {
            if let Some(budget) = &self.budget {
                let run_cost = self.ledger.known_cost();
                if !budget.allows(run_cost) {
                    return Err(ModelError::Provider(budget.refusal(run_cost)));
                }
            }
            let reply = self.api.turn(request, on, cancel).await?;
            self.record(request, &reply)?;
            Ok(reply)
        })
    }
}

impl Metered {
    fn record(&self, request: &TurnRequest, reply: &Reply) -> Result<(), ModelError> {
        if let Some(calls) = &self.calls {
            calls(request, reply);
        }
        let u = reply.usage;
        let provider = crate::providers::find(self.provider);
        let vendor = provider.map_or("", |provider| provider.vendor());
        let searched =
            llm_providers::search_price(vendor).map_or(0.0, |price| domain::billing::searches_cost(price, u.searches));
        let cost = rates(vendor, &request.model, u.prompt())
            .map(|rate| rate.estimate(u.input, u.output, u.cache_read, u.cache_write) + searched);
        self.ledger.costs.lock().map_err(|_| ModelError::Provider("usage ledger unavailable".into()))?.push(cost);
        self.gate.emit(spent_by(
            crate::agent::current::agent(),
            ModelUsage {
                model: request.model.clone(),
                input: u.input,
                output: u.output,
                cache_read: u.cache_read,
                cache_write: u.cache_write,
                searches: u.searches,
                estimated_cost_usd: cost,
                basis: if provider.is_some_and(|provider| provider.access().subscription()) {
                    "subscription-api-equivalent"
                } else {
                    "api-reference-estimate"
                }
                .into(),
                price_date: PRICE_DATE.into(),
            },
        ));
        Ok(())
    }
}
fn spent_by(agent: Option<usize>, usage: ModelUsage) -> EngineEvent {
    let spent = EngineEvent::Usage(usage);
    match agent {
        Some(index) => EngineEvent::AgentEvent { index, event: Box::new(spent) },
        None => spent,
    }
}

impl std::fmt::Debug for Metered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metered").field("provider", &self.provider).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use domain::budget::Budget;

    use super::*;
    use crate::boundary::EventSink;
    use crate::model::BlockingTurn;
    use crate::model::scripted::ScriptedModel;
    use crate::transcript::Block;

    struct Quiet;

    impl EventSink for Quiet {
        fn emit(&self, _run: domain::RunId, _event: EngineEvent) {}
    }

    #[test]
    fn a_sub_agent_s_spend_reaches_the_caller_wrapped_as_its_agent_s_event_and_still_counts() {
        let usage = ModelUsage { estimated_cost_usd: Some(0.25), ..ModelUsage::default() };
        assert_eq!(spent_by(None, usage.clone()), EngineEvent::Usage(usage.clone()));
        let wrapped = spent_by(Some(2), usage.clone());
        assert_eq!(wrapped, EngineEvent::AgentEvent { index: 2, event: Box::new(EngineEvent::Usage(usage)) });
        assert!((domain::budget::spent(&wrapped) - 0.25).abs() < f64::EPSILON);
    }

    fn metered(budget: Option<Budget>) -> Metered {
        Metered {
            api: Arc::new(ScriptedModel::new([ScriptedModel::reply(vec![Block::Text("ok".into())])])),
            ledger: Arc::default(),
            provider: ModelProvider::named("scripted"),
            gate: RunGate::new(domain::RunId::new(), Arc::new(Quiet)),
            model: domain::ModelOption::new("m", "M"),
            calls: None,
            budget,
        }
    }

    #[test]
    fn a_spent_budget_stops_the_next_call_before_it_is_made() {
        let request = TurnRequest {
            model: "m".into(),
            system: String::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            effort: domain::Effort::Medium,
            max_tokens: 100,
        };
        let stop = CancellationToken::new();

        let spent = metered(Some(Budget { limit_usd: 1.0, spent_usd: 1.0 }));
        let refused = spent.turn_blocking(&request, &mut |_| {}, &stop).expect_err("refused");
        assert!(refused.to_string().contains("budget"), "{refused}");

        let within = metered(Some(Budget { limit_usd: 1.0, spent_usd: 0.5 }));
        assert!(within.turn_blocking(&request, &mut |_| {}, &stop).is_ok());
        assert!(metered(None).turn_blocking(&request, &mut |_| {}, &stop).is_ok());
    }
}
