//! What Copilot serves a turn with: a body built for the wire's own API
//! fitted to the model's output limit, thinking and effort.

use domain::ModelOption;
use serde_json::{Value, json};

use super::{Wire, wire};
use crate::ProviderError;

/// Fits a body built for the wire's own API to what Copilot serves:
/// output limits, thinking and effort from the model's catalog entry.
pub fn shape(model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
    let caps = model.and_then(|m| m.capabilities.clone()).unwrap_or_default();
    let limit = |asked: Option<u64>, fallback: u64| {
        let most = caps.max_output_tokens.map_or(fallback, u64::from);
        asked.map_or(most, |asked| asked.min(most))
    };
    match wire(model) {
        Wire::Chat => {
            body["max_tokens"] = json!(limit(body["max_tokens"].as_u64(), 16_384));
        }
        Wire::Responses => {
            if caps.reasoning || !caps.reasoning_levels.is_empty() {
                body["reasoning"] = json!({"effort": level(&caps.reasoning_levels, effort), "summary": "auto"});
            } else if let Some(object) = body.as_object_mut() {
                object.remove("reasoning");
                object.remove("include");
            }
        }
        // The Copilot CLI's `getAnthropicRequestParams`: a model with
        // reasoning effort thinks adaptively at that effort; one that only
        // thinks gets a fixed budget; any other does not think.
        Wire::Messages => {
            let max = limit(body["max_tokens"].as_u64(), 8192);
            body["max_tokens"] = json!(max);
            if let Some(object) = body.as_object_mut() {
                object.remove("output_config");
                object.remove("thinking");
            }
            if caps.adaptive_thinking || !caps.reasoning_levels.is_empty() {
                let effort = if caps.reasoning_levels.is_empty() {
                    effort.to_owned()
                } else {
                    level(&caps.reasoning_levels, effort)
                };
                body["thinking"] = json!({"type": "adaptive", "display": "summarized"});
                body["output_config"] = json!({"effort": effort});
            } else if caps.reasoning {
                let max = max.max(BUDGET_FLOOR + 1);
                body["max_tokens"] = json!(max);
                body["thinking"] =
                    json!({"type": "enabled", "budget_tokens": budget(effort).clamp(BUDGET_FLOOR, max - 1)});
            }
        }
    }
    Ok(())
}

/// The asked effort, or the strongest level the model offers below it.
fn level(levels: &[String], effort: &str) -> String {
    const ORDER: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
    let asked = ORDER.iter().position(|l| *l == effort).unwrap_or(1);
    if levels.is_empty() {
        return ORDER[asked.min(2)].to_owned();
    }
    ORDER[..=asked]
        .iter()
        .rev()
        .find(|l| levels.iter().any(|offered| offered == *l))
        .map_or_else(|| levels[0].clone(), |l| (*l).to_owned())
}

const BUDGET_FLOOR: u64 = 1024;

/// The Copilot CLI's `reasoningEffortToBudget`.
fn budget(effort: &str) -> u64 {
    match effort {
        "low" => BUDGET_FLOOR,
        "medium" => BUDGET_FLOOR * 2,
        "xhigh" | "max" => 32_000,
        _ => BUDGET_FLOOR * 4,
    }
}

#[cfg(test)]
mod tests {
    use domain::model::ModelCapabilities;

    use super::*;
    use crate::copilot::served;

    fn shaped_messages(caps: ModelCapabilities, effort: &str) -> Value {
        let mut body = json!({"max_tokens": 64000, "thinking": {"type": "adaptive", "display": "summarized"},
            "output_config": {"effort": "low"}});
        shape(Some(&served(&["/v1/messages"], caps)), &mut body, effort).unwrap_or_else(|e| panic!("{e}"));
        body
    }

    #[test]
    fn claude_with_reasoning_effort_thinks_adaptively_at_an_offered_effort() {
        let levels = ["low", "medium", "high"].map(str::to_owned).to_vec();
        let body = shaped_messages(
            ModelCapabilities {
                reasoning_levels: levels,
                max_output_tokens: Some(32000),
                ..ModelCapabilities::default()
            },
            "max",
        );
        assert_eq!(
            body,
            json!({"max_tokens": 32000, "thinking": {"type": "adaptive", "display": "summarized"},
                "output_config": {"effort": "high"}})
        );
        let adaptive =
            shaped_messages(ModelCapabilities { adaptive_thinking: true, ..ModelCapabilities::default() }, "xhigh");
        assert_eq!(adaptive["output_config"], json!({"effort": "xhigh"}));
        assert_eq!(adaptive["max_tokens"], 8192);
    }

    #[test]
    fn claude_with_thinking_only_gets_the_copilot_cli_budget() {
        let thinking = ModelCapabilities { reasoning: true, ..ModelCapabilities::default() };
        let medium = shaped_messages(thinking.clone(), "medium");
        assert_eq!(medium["thinking"], json!({"type": "enabled", "budget_tokens": 2048}));
        assert!(medium.get("output_config").is_none());
        assert_eq!(shaped_messages(thinking.clone(), "low")["thinking"]["budget_tokens"], 1024);
        assert_eq!(shaped_messages(thinking, "max")["thinking"]["budget_tokens"], 8191, "within max_tokens");
        let plain = shaped_messages(ModelCapabilities::default(), "high");
        assert_eq!(plain, json!({"max_tokens": 8192}));
    }

    #[test]
    fn responses_reason_at_the_nearest_offered_level() {
        let levels = vec!["low".to_owned(), "medium".to_owned(), "high".to_owned()];
        let model =
            served(&["/responses"], ModelCapabilities { reasoning_levels: levels, ..ModelCapabilities::default() });
        let mut body = json!({"reasoning": {"effort": "high"}});
        shape(Some(&model), &mut body, "max").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["reasoning"], json!({"effort": "high", "summary": "auto"}));
        let plain = served(&["/responses"], ModelCapabilities::default());
        let mut body = json!({"reasoning": {}, "include": ["reasoning.encrypted_content"]});
        shape(Some(&plain), &mut body, "high").unwrap_or_else(|e| panic!("{e}"));
        assert!(body.get("reasoning").is_none() && body.get("include").is_none());
    }

    #[test]
    fn chat_stays_within_the_model_output_limit() {
        let model = served(&[], ModelCapabilities { max_output_tokens: Some(4096), ..ModelCapabilities::default() });
        let mut body = json!({"max_tokens": 64000});
        shape(Some(&model), &mut body, "medium").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["max_tokens"], 4096);
    }
}
