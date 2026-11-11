//! What each LLM provider changes in a streamed request before it is
//! sent: the model's name, the system prompt, the limits and the effort.
use super::ProviderError;
use super::catalog::REASONING_PREFIXES;
use super::copilot;
use domain::{ModelOption, ModelProvider};
use serde_json::{Value, json};

pub const STREAM_LIMIT: usize = 8 << 20;
pub const ERROR_LIMIT: usize = 4 << 10;

pub fn path(provider: ModelProvider, model: Option<&ModelOption>) -> &'static str {
    match provider {
        ModelProvider::ChatGptSubscription => "/codex/responses",
        ModelProvider::CopilotSubscription => copilot::path(copilot::wire(model)),
        ModelProvider::OpenAi => "/responses",
        ModelProvider::KiroSubscription => "/",
        _ => "/messages",
    }
}

/// What the account adds to a shaped turn: Kiro's profile ARN.
pub fn bind(provider: ModelProvider, config: &super::Config, body: &mut Value) {
    if provider == ModelProvider::KiroSubscription
        && let Some(tokens) = &config.auth
    {
        super::kiro::bind(body, &tokens.account_id);
    }
}

/// What a turn adds to the account's headers, read from its shaped body.
pub fn turn_headers(provider: ModelProvider, model: Option<&ModelOption>, body: &Value) -> Vec<(String, String)> {
    if provider == ModelProvider::CopilotSubscription {
        copilot::turn_headers(copilot::wire(model), body)
    } else {
        Vec::new()
    }
}

pub fn retries_after_refresh(provider: ModelProvider, status: u16) -> bool {
    match provider {
        ModelProvider::ChatGptSubscription => status == 401,
        ModelProvider::KiroSubscription => matches!(status, 401 | 403),
        _ => false,
    }
}

pub fn shape(provider: ModelProvider, model: Option<&ModelOption>, body: &mut Value) -> Result<(), ProviderError> {
    let prefix = format!("{}:", provider.id());
    if let Some(name) = body["model"].as_str() {
        body["model"] = json!(name.strip_prefix(&prefix).unwrap_or(name));
    }
    let effort = body["reasoning"]["effort"]
        .as_str()
        .or_else(|| body["output_config"]["effort"].as_str())
        .unwrap_or("medium")
        .to_owned();
    match provider {
        ModelProvider::ChatGptSubscription => codex(body, &effort),
        ModelProvider::OpenAi => remove(body, &["reasoning", "tool_choice"]),
        ModelProvider::Anthropic => anthropic(body, model),
        ModelProvider::ClaudeSubscription => claude_subscription(body),
        ModelProvider::CopilotSubscription => return copilot::shape(model, body, &effort),
        ModelProvider::KiroSubscription => {
            super::kiro::shape(body);
            return Ok(());
        }
    }
    apply_effort(body, provider, model, &effort)
}

fn remove(body: &mut Value, keys: &[&str]) {
    if let Some(object) = body.as_object_mut() {
        for key in keys {
            object.remove(*key);
        }
    }
}

fn codex(body: &mut Value, effort: &str) {
    body["instructions"] = json!(format!(
        "You are a helpful coding assistant operating through {brand_name}.\n\n{}",
        body["instructions"].as_str().unwrap_or_default(),
        brand_name = brand::NAME
    ));
    body["reasoning"]["effort"] = json!(effort);
    body["text"] = json!({"verbosity": "medium"});
    remove(body, &["parallel_tool_calls", "tool_choice"]);
    if let Some(tools) = body["tools"].as_array_mut() {
        for tool in tools {
            if let Some(tool) = tool.as_object_mut() {
                tool.remove("strict");
            }
        }
    }
}

fn anthropic(body: &mut Value, model: Option<&ModelOption>) {
    let caps = model.and_then(|model| model.capabilities.as_ref());
    let max_tokens = caps.and_then(|c| c.max_output_tokens).unwrap_or(8192).min(8192);
    body["max_tokens"] = json!(max_tokens);
    body["system"] = json!(
        body["system"]
            .as_array()
            .map(|blocks| blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n\n"))
            .unwrap_or_default()
    );
    remove(body, &["thinking", "output_config", "cache_control"]);
    if let Some(caps) = caps.filter(|c| c.reasoning && (c.adaptive_thinking || max_tokens > 1)) {
        body["thinking"] = if caps.adaptive_thinking {
            json!({"type":"adaptive"})
        } else {
            json!({"type":"enabled", "budget_tokens": if max_tokens <= 4096 { max_tokens / 2 } else { 4096 }})
        };
    }
}

fn claude_subscription(body: &mut Value) {
    let mut blocks = vec![json!({"type":"text", "text":"You are Claude Code, Anthropic's official CLI for Claude."})];
    if let Some(existing) = body["system"].as_array() {
        blocks.extend(existing.iter().cloned());
    }
    body["system"] = json!(blocks);
    body["max_tokens"] = json!(8192);
    remove(body, &["thinking", "output_config", "cache_control"]);
}

fn apply_effort(
    body: &mut Value,
    provider: ModelProvider,
    selected: Option<&ModelOption>,
    effort: &str,
) -> Result<(), ProviderError> {
    let model = body["model"].as_str().unwrap_or_default().to_owned();
    let caps =
        selected.filter(|s| s.id.rsplit(':').next() == Some(model.as_str())).and_then(|s| s.capabilities.as_ref());
    let unsupported =
        |what: &str| caps.filter(|c| !c.reasoning_levels.is_empty() && !c.reasoning_levels.iter().any(|l| l == what));
    if matches!(provider, ModelProvider::OpenAi | ModelProvider::ChatGptSubscription) {
        let supports = provider == ModelProvider::ChatGptSubscription
            || caps.is_some_and(|c| c.reasoning)
            || REASONING_PREFIXES.iter().any(|prefix| model.starts_with(prefix));
        if supports {
            let effort = if effort == "max" { "xhigh" } else { effort };
            if unsupported(effort).is_some() {
                return Err(ProviderError::Invalid(format!(
                    "model {model} does not support reasoning effort {effort}"
                )));
            }
            body["reasoning"] = json!({"effort":effort,"summary":"auto"});
        }
        return Ok(());
    }
    let adaptive =
        is_modern_claude(&model) || caps.is_some_and(|c| c.adaptive_thinking) || is_adaptive_claude_4(&model);
    let supports = adaptive || model.starts_with("claude-opus-4-5");
    if supports {
        if unsupported(effort).is_some() {
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

    fn shaped(provider: ModelProvider, model: Option<&ModelOption>, body: Value) -> Value {
        let mut body = body;
        shape(provider, model, &mut body).unwrap_or_else(|e| panic!("{e}"));
        body
    }

    #[test]
    fn the_provider_prefix_leaves_the_model_name() {
        let body = shaped(ModelProvider::OpenAi, None, json!({"model": "openai:gpt-4o"}));
        assert_eq!(body["model"], "gpt-4o");
        assert!(body.get("reasoning").is_none());
    }

    #[test]
    fn codex_gets_its_instructions_effort_and_no_strict_tools() {
        let body = shaped(
            ModelProvider::ChatGptSubscription,
            None,
            json!({"model": "openai-subscription:gpt-5", "instructions": "be brief",
                "reasoning": {"effort": "max"}, "tool_choice": "auto", "tools": [{"name": "t", "strict": true}]}),
        );
        assert!(body["instructions"].as_str().is_some_and(|s| s.ends_with("be brief")));
        assert_eq!(body["reasoning"], json!({"effort": "xhigh", "summary": "auto"}));
        assert!(body.get("tool_choice").is_none() && body["tools"][0].get("strict").is_none());
    }

    #[test]
    fn claude_subscription_speaks_as_claude_code() {
        let body = shaped(
            ModelProvider::ClaudeSubscription,
            None,
            json!({"model": "claude-subscription:claude-sonnet-4-5", "system": [{"type": "text", "text": "x"}]}),
        );
        assert_eq!(body["system"][0]["text"], "You are Claude Code, Anthropic's official CLI for Claude.");
        assert_eq!(body["max_tokens"], 8192);
    }

    #[test]
    fn anthropic_flattens_the_system_and_thinks_by_capability() {
        let mut model = ModelOption::new("anthropic:claude-opus-4-1", "Opus");
        model.capabilities =
            Some(ModelCapabilities { reasoning: true, max_output_tokens: Some(32000), ..ModelCapabilities::default() });
        let body = shaped(
            ModelProvider::Anthropic,
            Some(&model),
            json!({"model": "anthropic:claude-opus-4-1", "system": [{"text": "a"}, {"text": "b"}],
                "output_config": {"effort": "high"}}),
        );
        assert_eq!(body["system"], "a\n\nb");
        assert_eq!(body["thinking"], json!({"type": "enabled", "budget_tokens": 4096}));
    }

    #[test]
    fn modern_claude_models_think_adaptively_with_the_effort() {
        for model in ["claude-opus-5", "claude-sonnet-5-5", "claude-opus-4-6"] {
            let body = shaped(
                ModelProvider::ClaudeSubscription,
                None,
                json!({"model": model, "output_config": {"effort": "low"}}),
            );
            assert_eq!(body["thinking"], json!({"type": "adaptive"}), "{model}");
            assert_eq!(body["output_config"], json!({"effort": "low"}));
        }
        assert!(!is_modern_claude("claude-opus-50") && !is_adaptive_claude_4("claude-opus-4-5"));
    }

    #[test]
    fn an_effort_outside_the_catalog_is_refused() {
        let mut model = ModelOption::new("openai-subscription:gpt-5", "GPT-5");
        model.capabilities = Some(ModelCapabilities {
            reasoning: true,
            reasoning_levels: vec!["low".into()],
            ..ModelCapabilities::default()
        });
        let mut body = json!({"model": "openai-subscription:gpt-5", "reasoning": {"effort": "high"}});
        assert!(shape(ModelProvider::ChatGptSubscription, Some(&model), &mut body).is_err());
    }
}
