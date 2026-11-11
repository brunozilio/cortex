use domain::ModelOption;
use domain::billing::TokenRates;
use serde_json::{Value, json};

use crate::request::{capabilities, remove, unsupported};
use crate::vendor::{Price, per, rated};
use crate::{ProviderError, Vendor as Family};

#[derive(Debug, Clone, Copy, Default)]
pub struct Vendor;

const PRICES: &[Price] = &[
    (&["claude-opus-5-5"], per(4_000_000, 20_000_000, 200_000, 5_000_000)),
    (&["claude-sonnet-5-5"], per(2_000_000, 10_000_000, 100_000, 2_500_000)),
    (&["claude-fable-5-1", "claude-mythos-5-1"], per(10_000_000, 50_000_000, 250_000, 12_500_000)),
    (&["claude-sonnet-4-5", "claude-sonnet-4-6"], per(3_000_000, 15_000_000, 300_000, 3_750_000)),
    (
        &["claude-opus-4-5", "claude-opus-4-6", "claude-opus-4-7", "claude-opus-4-8", "claude-opus-5"],
        per(5_000_000, 25_000_000, 500_000, 6_250_000),
    ),
    (&["claude-haiku-4-5"], per(1_000_000, 5_000_000, 100_000, 1_250_000)),
    (&["claude-opus-4-1"], per(15_000_000, 75_000_000, 1_500_000, 18_750_000)),
];

impl Family for Vendor {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    fn owns(&self, model: &str) -> bool {
        model.starts_with("claude-")
    }

    fn rates(&self, model: &str, prompt_tokens: u64) -> Option<TokenRates> {
        match model {
            "claude-haiku-5-5" if prompt_tokens <= 100_000 => Some(per(100_000, 500_000, 10_000, 125_000)),
            "claude-haiku-5-5" => Some(per(500_000, 2_500_000, 50_000, 625_000)),
            _ => rated(PRICES, model),
        }
    }

    fn alias(&self, name: &str) -> Option<&'static str> {
        match name {
            "sonnet" => Some("claude-sonnet-5-5"),
            "opus" => Some("claude-opus-5-5"),
            "haiku" => Some("claude-haiku-5-5"),
            _ => None,
        }
    }

    fn effort(
        &self,
        body: &mut Value,
        selected: Option<&ModelOption>,
        effort: &str,
        _always: bool,
    ) -> Result<(), ProviderError> {
        let caps = capabilities(body, selected);
        let model = body["model"].as_str().unwrap_or_default().to_owned();
        let adaptive = modern(&model) || caps.is_some_and(|c| c.adaptive_thinking) || adaptive_4(&model);
        let supports = adaptive || model.starts_with("claude-opus-4-5");
        if supports {
            if unsupported(caps, effort) {
                return Err(ProviderError::Invalid(format!("model {model} does not support effort {effort}")));
            }
            body["output_config"] = json!({"effort":effort});
            if adaptive {
                body["thinking"] = json!({"type":"adaptive"});
            }
        } else if body["thinking"]["type"] == "enabled" {
            let max = body["max_tokens"].as_u64().unwrap_or(8192);
            let budget = match effort {
                "low" => max / 8,
                "medium" => max / 4,
                "high" => max / 2,
                "xhigh" => max * 3 / 4,
                _ => max * 7 / 8,
            }
            .max(1024);
            if max > 1024 {
                body["thinking"]["budget_tokens"] = json!(budget.min(max - 1));
            } else {
                remove(body, &["thinking"]);
            }
        }
        Ok(())
    }
}

fn modern(model: &str) -> bool {
    ["opus", "sonnet", "haiku"].iter().any(|family| {
        model
            .strip_prefix("claude-")
            .and_then(|rest| rest.strip_prefix(family))
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|rest| version_digit(rest, '5'..='9'))
    })
}

fn adaptive_4(model: &str) -> bool {
    ["opus", "sonnet"].iter().any(|family| {
        model
            .strip_prefix("claude-")
            .and_then(|rest| rest.strip_prefix(family))
            .and_then(|rest| rest.strip_prefix("-4-"))
            .is_some_and(|rest| version_digit(rest, '6'..='9'))
    })
}

fn version_digit(rest: &str, range: std::ops::RangeInclusive<char>) -> bool {
    let mut chars = rest.chars();
    chars.next().is_some_and(|c| range.contains(&c)) && matches!(chars.next(), None | Some('-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_claude_models_think_adaptively_with_the_effort() {
        for model in ["claude-opus-5", "claude-sonnet-5-5", "claude-opus-4-6"] {
            let mut body = json!({"model": model});
            Vendor.effort(&mut body, None, "low", false).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(body["thinking"], json!({"type": "adaptive"}), "{model}");
            assert_eq!(body["output_config"], json!({"effort": "low"}));
        }
        assert!(!modern("claude-opus-50") && !adaptive_4("claude-opus-4-5"));
    }

    #[test]
    fn prices_are_known_per_model_and_by_prompt_size() {
        let rate = Vendor.rates("claude-sonnet-5-5", 100).expect("priced");
        assert!((rate.estimate(100, 10, 20, 5) - 0.0003145).abs() < 1e-12);
        let input = |tokens| Vendor.rates("claude-haiku-5-5", tokens).map(|rate| rate.input);
        assert_eq!((input(100), input(100_001)), (Some(100_000), Some(500_000)));
        assert!(Vendor.rates("unreviewed-model", 1).is_none());
    }

    #[test]
    fn short_names_pick_the_current_models() {
        assert_eq!(Vendor.alias("opus"), Some("claude-opus-5-5"));
        assert_eq!(Vendor.alias("gpt"), None);
    }
}
