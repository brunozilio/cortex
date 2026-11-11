use super::{Provider, ProviderError};
use domain::ModelOption;
use serde_json::{Value, json};

pub const STREAM_LIMIT: usize = 8 << 20;
pub const ERROR_LIMIT: usize = 4 << 10;
pub const REASONING_PREFIXES: [&str; 5] = ["o1", "o3", "o4", "gpt-5", "gpt-6"];

pub fn shape(provider: &dyn Provider, model: Option<&ModelOption>, body: &mut Value) -> Result<(), ProviderError> {
    let prefix = format!("{}:", provider.id());
    if let Some(name) = body["model"].as_str() {
        body["model"] = json!(name.strip_prefix(&prefix).unwrap_or(name));
    }
    let effort = body["reasoning"]["effort"]
        .as_str()
        .or_else(|| body["output_config"]["effort"].as_str())
        .unwrap_or("medium")
        .to_owned();
    provider.shape(model, body, &effort)
}

pub fn remove(body: &mut Value, keys: &[&str]) {
    if let Some(object) = body.as_object_mut() {
        for key in keys {
            object.remove(*key);
        }
    }
}

fn capabilities<'a>(body: &Value, selected: Option<&'a ModelOption>) -> Option<&'a domain::model::ModelCapabilities> {
    let model = body["model"].as_str().unwrap_or_default();
    selected.filter(|s| s.id.rsplit(':').next() == Some(model)).and_then(|s| s.capabilities.as_ref())
}

fn unsupported(caps: Option<&domain::model::ModelCapabilities>, what: &str) -> bool {
    caps.is_some_and(|c| !c.reasoning_levels.is_empty() && !c.reasoning_levels.iter().any(|l| l == what))
}

pub fn responses_effort(
    body: &mut Value,
    selected: Option<&ModelOption>,
    effort: &str,
    always: bool,
) -> Result<(), ProviderError> {
    let caps = capabilities(body, selected);
    let model = body["model"].as_str().unwrap_or_default().to_owned();
    let supports = always
        || caps.is_some_and(|c| c.reasoning)
        || REASONING_PREFIXES.iter().any(|prefix| model.starts_with(prefix));
    if supports {
        let effort = if effort == "max" { "xhigh" } else { effort };
        if unsupported(caps, effort) {
            return Err(ProviderError::Invalid(format!("model {model} does not support reasoning effort {effort}")));
        }
        body["reasoning"] = json!({"effort":effort,"summary":"auto"});
    }
    Ok(())
}

pub fn messages_effort(body: &mut Value, selected: Option<&ModelOption>, effort: &str) -> Result<(), ProviderError> {
    let caps = capabilities(body, selected);
    let model = body["model"].as_str().unwrap_or_default().to_owned();
    let adaptive =
        is_modern_claude(&model) || caps.is_some_and(|c| c.adaptive_thinking) || is_adaptive_claude_4(&model);
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

fn is_modern_claude(model: &str) -> bool {
    ["opus", "sonnet", "haiku"].iter().any(|family| {
        model
            .strip_prefix("claude-")
            .and_then(|rest| rest.strip_prefix(family))
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|rest| version_digit(rest, '5'..='9'))
    })
}

fn is_adaptive_claude_4(model: &str) -> bool {
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
    use domain::model::ModelCapabilities;

    #[test]
    fn modern_claude_models_think_adaptively_with_the_effort() {
        for model in ["claude-opus-5", "claude-sonnet-5-5", "claude-opus-4-6"] {
            let mut body = json!({"model": model});
            messages_effort(&mut body, None, "low").unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(body["thinking"], json!({"type": "adaptive"}), "{model}");
            assert_eq!(body["output_config"], json!({"effort": "low"}));
        }
        assert!(!is_modern_claude("claude-opus-50") && !is_adaptive_claude_4("claude-opus-4-5"));
    }

    #[test]
    fn an_effort_outside_the_catalog_is_refused() {
        let mut model = ModelOption::new("x:gpt-5", "GPT-5");
        model.capabilities = Some(ModelCapabilities {
            reasoning: true,
            reasoning_levels: vec!["low".into()],
            ..ModelCapabilities::default()
        });
        let mut body = json!({"model": "gpt-5"});
        assert!(responses_effort(&mut body, Some(&model), "high", true).is_err());
        assert!(responses_effort(&mut body, Some(&model), "low", true).is_ok());
        assert_eq!(body["reasoning"], json!({"effort": "low", "summary": "auto"}));
    }

    #[test]
    fn the_provider_prefix_leaves_the_model_name() {
        let mut body = json!({"model": "keyed:gpt-4o"});
        shape(&crate::testing::KEYED, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["model"], "gpt-4o");
    }
}
