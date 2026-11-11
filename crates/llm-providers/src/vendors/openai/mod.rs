use domain::ModelOption;
use domain::billing::TokenRates;
use serde_json::{Value, json};

use crate::request::{capabilities, unsupported};
use crate::vendor::{Price, per, rated};
use crate::{ProviderError, Vendor as Family};

#[derive(Debug, Clone, Copy, Default)]
pub struct Vendor;

const REASONING: [&str; 5] = ["o1", "o3", "o4", "gpt-5", "gpt-6"];

const PRICES: &[Price] = &[
    (&["gpt-6-astra"], per(10_000_000, 50_000_000, 1_000_000, 12_500_000)),
    (&["gpt-6.1-sol"], per(2_000_000, 10_000_000, 100_000, 2_500_000)),
    (&["gpt-6.1-terra"], per(1_000_000, 5_000_000, 50_000, 1_250_000)),
    (&["gpt-6-luna"], per(100_000, 500_000, 10_000, 125_000)),
    (&["gpt-5.3-codex"], per(1_750_000, 14_000_000, 175_000, 0)),
];

impl Family for Vendor {
    fn id(&self) -> &'static str {
        "openai"
    }

    fn owns(&self, model: &str) -> bool {
        model.starts_with("gpt-") || ["o1", "o3", "o4"].iter().any(|prefix| model.starts_with(prefix))
    }

    fn reasons(&self, model: &str) -> bool {
        let lower = model.to_ascii_lowercase();
        REASONING.iter().any(|prefix| lower.starts_with(prefix))
    }

    fn rates(&self, model: &str, _prompt_tokens: u64) -> Option<TokenRates> {
        rated(PRICES, model)
    }

    fn effort(
        &self,
        body: &mut Value,
        selected: Option<&ModelOption>,
        effort: &str,
        always: bool,
    ) -> Result<(), ProviderError> {
        let caps = capabilities(body, selected);
        let model = body["model"].as_str().unwrap_or_default().to_owned();
        let supports = always || caps.is_some_and(|c| c.reasoning) || self.reasons(&model);
        if supports {
            let effort = if effort == "max" { "xhigh" } else { effort };
            if unsupported(caps, effort) {
                return Err(ProviderError::Invalid(format!(
                    "model {model} does not support reasoning effort {effort}"
                )));
            }
            body["reasoning"] = json!({"effort":effort,"summary":"auto"});
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::model::ModelCapabilities;

    #[test]
    fn an_effort_outside_the_catalog_is_refused() {
        let mut model = ModelOption::new("x:gpt-5", "GPT-5");
        model.capabilities = Some(ModelCapabilities {
            reasoning: true,
            reasoning_levels: vec!["low".into()],
            ..ModelCapabilities::default()
        });
        let mut body = json!({"model": "gpt-5"});
        assert!(Vendor.effort(&mut body, Some(&model), "high", true).is_err());
        assert!(Vendor.effort(&mut body, Some(&model), "low", true).is_ok());
        assert_eq!(body["reasoning"], json!({"effort": "low", "summary": "auto"}));
    }

    #[test]
    fn its_models_are_priced_and_reasoning_ones_known() {
        assert!(Vendor.rates("gpt-6.1-sol", 1).is_some());
        assert!(Vendor.owns("o3-mini") && !Vendor.owns("claude-opus-5-5"));
        assert!(Vendor.reasons("GPT-5-mini") && !Vendor.reasons("gpt-4o"));
    }
}
