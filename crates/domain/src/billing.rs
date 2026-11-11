use crate::ModelProvider;
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

pub fn rates(provider: ModelProvider, model: &str, prompt_tokens: u64) -> Option<TokenRates> {
    let model = model.rsplit(':').next().unwrap_or(model);
    let tuple = match model {
        "claude-opus-5-5" => (4_000_000, 20_000_000, 200_000, 5_000_000),
        "claude-sonnet-5-5" => (2_000_000, 10_000_000, 100_000, 2_500_000),
        "claude-fable-5-1" | "claude-mythos-5-1" => (10_000_000, 50_000_000, 250_000, 12_500_000),
        "claude-haiku-5-5" if prompt_tokens <= 100_000 => (100_000, 500_000, 10_000, 125_000),
        "claude-haiku-5-5" => (500_000, 2_500_000, 50_000, 625_000),
        "claude-sonnet-4-5" | "claude-sonnet-4-6" => (3_000_000, 15_000_000, 300_000, 3_750_000),
        "claude-opus-4-5" | "claude-opus-4-6" | "claude-opus-4-7" | "claude-opus-4-8" | "claude-opus-5" => {
            (5_000_000, 25_000_000, 500_000, 6_250_000)
        }
        "claude-haiku-4-5" => (1_000_000, 5_000_000, 100_000, 1_250_000),
        "claude-opus-4-1" => (15_000_000, 75_000_000, 1_500_000, 18_750_000),
        "gpt-6-astra" => (10_000_000, 50_000_000, 1_000_000, 12_500_000),
        "gpt-6.1-sol" => (2_000_000, 10_000_000, 100_000, 2_500_000),
        "gpt-6.1-terra" => (1_000_000, 5_000_000, 50_000, 1_250_000),
        "gpt-6-luna" => (100_000, 500_000, 10_000, 125_000),
        "gpt-5.3-codex" => (1_750_000, 14_000_000, 175_000, 0),
        _ => return None,
    };
    let matches_provider = if model.starts_with("claude-") {
        matches!(provider, ModelProvider::Anthropic | ModelProvider::ClaudeSubscription)
    } else {
        matches!(provider, ModelProvider::OpenAi | ModelProvider::ChatGptSubscription)
    };
    matches_provider.then_some(TokenRates {
        input: tuple.0,
        output: tuple.1,
        cache_read: tuple.2,
        cache_write: tuple.3,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_unknown_provider_and_cache_rates_are_explicit() {
        let rate = rates(ModelProvider::Anthropic, "claude-sonnet-5-5", 100).expect("valid test fixture");
        assert!((rate.estimate(100, 10, 20, 5) - 0.0003145).abs() < 1e-12);
        assert!(rates(ModelProvider::OpenAi, "claude-sonnet-5-5", 1).is_none());
        assert!(rates(ModelProvider::Anthropic, "unreviewed-model", 1).is_none());
    }
}
