use domain::ModelOption;
use domain::billing::TokenRates;
use serde_json::{Value, json};

use crate::request::{capabilities, remove};
use crate::{ProviderError, Vendor as Family};

#[derive(Debug, Clone, Copy, Default)]
pub struct Vendor;

const WITH_EFFORT: [&str; 5] = ["grok-3-mini", "grok-4.20-multi-agent", "grok-4.3", "grok-4.5", "grok-4.6"];
const LEVELS: [&str; 3] = ["low", "medium", "high"];
const EXTRA_HIGH: [&str; 1] = ["grok-4.6"];

impl Family for Vendor {
    fn id(&self) -> &'static str {
        "xai"
    }

    fn owns(&self, model: &str) -> bool {
        model.to_ascii_lowercase().starts_with("grok")
    }

    fn reasons(&self, model: &str) -> bool {
        let lower = model.to_ascii_lowercase();
        WITH_EFFORT.iter().any(|prefix| lower.starts_with(prefix))
    }

    fn rates(&self, _model: &str, _prompt_tokens: u64) -> Option<TokenRates> {
        None
    }

    fn effort(
        &self,
        body: &mut Value,
        selected: Option<&ModelOption>,
        effort: &str,
        _always: bool,
    ) -> Result<(), ProviderError> {
        let model = body["model"].as_str().unwrap_or_default().to_owned();
        let offered: Vec<String> = capabilities(body, selected)
            .filter(|caps| caps.reasoning)
            .map(|caps| caps.reasoning_levels.clone())
            .unwrap_or_default();
        if offered.is_empty() && !self.reasons(&model) {
            remove(body, &["reasoning"]);
            return Ok(());
        }
        let levels = if offered.is_empty() { known_levels(&model) } else { offered };
        body["reasoning"] = json!({"effort": nearest(&levels, effort)});
        Ok(())
    }
}

fn known_levels(model: &str) -> Vec<String> {
    let lower = model.to_ascii_lowercase();
    let extra = EXTRA_HIGH.iter().any(|family| lower == *family || lower.starts_with(&format!("{family}-")));
    LEVELS.iter().chain(extra.then_some(&"xhigh")).map(|level| (*level).to_owned()).collect()
}

fn nearest(levels: &[String], effort: &str) -> String {
    let ladder = ["low", "medium", "high", "xhigh", "max"];
    let wanted = ladder.iter().position(|level| *level == effort).unwrap_or(1);
    ladder[..=wanted]
        .iter()
        .rev()
        .find(|level| levels.iter().any(|offered| offered == *level))
        .or_else(|| ladder.iter().find(|level| levels.iter().any(|offered| offered == *level)))
        .map_or_else(|| "medium".to_owned(), |level| (*level).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::model::ModelCapabilities;

    fn sent(model: &str, effort: &str) -> Value {
        let mut body = json!({"model": model, "reasoning": {"effort": effort, "summary": "auto"}});
        Vendor.effort(&mut body, None, effort, true).unwrap_or_else(|e| panic!("{e}"));
        body
    }

    #[test]
    fn only_models_with_an_effort_dial_get_one_and_never_a_summary() {
        assert_eq!(sent("grok-4.3", "medium")["reasoning"], json!({"effort": "medium"}));
        assert!(sent("grok-4.20-0309-non-reasoning", "high").get("reasoning").is_none());
        assert!(sent("grok-composer-2.5-fast", "high").get("reasoning").is_none());
    }

    #[test]
    fn the_highest_efforts_fall_to_what_the_family_accepts() {
        assert_eq!(sent("grok-4.6", "max")["reasoning"]["effort"], "xhigh");
        assert_eq!(sent("grok-4.6-fast", "xhigh")["reasoning"]["effort"], "xhigh");
        assert_eq!(sent("grok-4.5", "xhigh")["reasoning"]["effort"], "high");
        assert_eq!(sent("grok-4.3", "low")["reasoning"]["effort"], "low");
    }

    #[test]
    fn the_catalog_s_levels_win_over_the_known_families() {
        let mut model = ModelOption::new("x:grok-9", "Grok 9");
        model.capabilities = Some(ModelCapabilities {
            reasoning: true,
            reasoning_levels: vec!["medium".into(), "high".into()],
            ..ModelCapabilities::default()
        });
        let mut body = json!({"model": "grok-9"});
        Vendor.effort(&mut body, Some(&model), "low", true).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["reasoning"], json!({"effort": "medium"}));
    }

    #[test]
    fn it_owns_grok_models_only() {
        assert!(Vendor.owns("grok-4.6") && !Vendor.owns("gpt-5"));
        assert!(Vendor.reasons("grok-4.5") && !Vendor.reasons("grok-4.20-0309-non-reasoning"));
    }
}
