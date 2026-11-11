use serde::{Deserialize, Serialize};

pub const PRICE_DATE: &str = "2026-11-07";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenRates {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}
impl TokenRates {
    pub fn estimate(self, input: u64, output: u64, cache_read: u64, cache_write: u64) -> f64 {
        let micro = u128::from(input) * u128::from(self.input)
            + u128::from(output) * u128::from(self.output)
            + u128::from(cache_read) * u128::from(self.cache_read)
            + u128::from(cache_write) * u128::from(self.cache_write);
        micro as f64 / 1_000_000_000_000.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub estimated_cost_usd: Option<f64>,
    pub basis: String,
    pub price_date: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionUsage {
    pub models: Vec<ModelUsage>,
}

impl SessionUsage {
    pub fn add(&mut self, call: &ModelUsage) {
        let Some(kept) = self.models.iter_mut().find(|kept| kept.model == call.model) else {
            return self.models.push(call.clone());
        };
        kept.input += call.input;
        kept.output += call.output;
        kept.cache_read += call.cache_read;
        kept.cache_write += call.cache_write;
        kept.estimated_cost_usd = match (kept.estimated_cost_usd, call.estimated_cost_usd) {
            (None, None) => None,
            (kept, call) => Some(kept.unwrap_or(0.0) + call.unwrap_or(0.0)),
        };
    }
}

pub fn cache_hit_rate(usage: &ModelUsage) -> Option<f64> {
    let prompt = usage.input + usage.cache_read + usage.cache_write;
    (prompt > 0).then(|| usage.cache_read as f64 / prompt as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(model: &str, input: u64, cache_read: u64, cost: Option<f64>) -> ModelUsage {
        ModelUsage {
            model: model.into(),
            input,
            output: 10,
            cache_read,
            cache_write: 0,
            estimated_cost_usd: cost,
            basis: String::new(),
            price_date: String::new(),
        }
    }

    #[test]
    fn a_session_adds_up_its_calls_per_model() {
        let mut session = SessionUsage::default();
        session.add(&call("opus", 100, 900, Some(0.5)));
        session.add(&call("haiku", 50, 0, None));
        session.add(&call("opus", 100, 900, Some(0.25)));
        assert_eq!(session.models.len(), 2);
        let opus = &session.models[0];
        assert_eq!((opus.model.as_str(), opus.input, opus.output, opus.cache_read), ("opus", 200, 20, 1800));
        assert_eq!(opus.estimated_cost_usd, Some(0.75));
        assert_eq!(session.models[1].estimated_cost_usd, None, "an unpriced model stays unpriced");
    }

    #[test]
    fn the_cache_hit_rate_is_the_share_of_prompt_tokens_read_from_cache() {
        assert_eq!(cache_hit_rate(&call("opus", 100, 900, None)), Some(0.9));
        assert_eq!(cache_hit_rate(&call("opus", 0, 0, None)), None);
    }
}
