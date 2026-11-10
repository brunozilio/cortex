//! The GitHub Copilot subscription as the Copilot CLI (`@github/copilot`
//! 1.0.62, checked against 1.0.108) speaks it: the GitHub token sent as is
//! to the Copilot API, each model served on the API its
//! `supported_endpoints` name, and the headers that bill a premium request
//! only for a turn the user started.
pub mod device;
pub mod models;

use super::ProviderError;
use serde_json::{Value, json};
use superfik_core::ModelOption;

pub const API: &str = "https://api.githubcopilot.com";
pub const VERSION: &str = "1.0.62";
pub const API_VERSION: &str = "2026-06-01";
pub const INTEGRATION: &str = "copilot-developer-cli";

/// Which of the Copilot API's three wire formats a model is served on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    Messages,
    Responses,
    Chat,
}

/// Anthropic's Messages when offered, then Responses, else Chat
/// Completions: the Copilot CLI's order with thinking turned on.
pub fn wire(model: Option<&ModelOption>) -> Wire {
    let endpoints = model.and_then(|m| m.capabilities.as_ref()).map(|c| c.endpoints.as_slice()).unwrap_or_default();
    let offers = |path: &str| endpoints.iter().any(|e| e == path);
    if offers("/v1/messages") {
        Wire::Messages
    } else if offers("/responses") {
        Wire::Responses
    } else {
        Wire::Chat
    }
}

pub const fn path(wire: Wire) -> &'static str {
    match wire {
        Wire::Messages => "/v1/messages",
        Wire::Responses => "/responses",
        Wire::Chat => "/chat/completions",
    }
}

pub fn user_agent() -> String {
    format!("copilot/{VERSION} ({}) term/unknown", std::env::consts::OS)
}

/// What every Copilot API call carries.
pub fn headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("authorization".into(), format!("Bearer {token}")),
        ("copilot-integration-id".into(), INTEGRATION.into()),
        ("editor-version".into(), format!("copilot/{VERSION}")),
        ("user-agent".into(), user_agent()),
        ("openai-intent".into(), "conversation-agent".into()),
        ("x-github-api-version".into(), API_VERSION.into()),
    ]
}

/// What one turn adds: who started it (only a user's turn is a premium
/// request), whether it carries images, and the Messages API version.
pub fn turn_headers(wire: Wire, body: &Value) -> Vec<(String, String)> {
    let by_user = started_by_user(body);
    let mut headers = vec![
        ("x-initiator".to_owned(), if by_user { "user" } else { "agent" }.to_owned()),
        ("x-interaction-type".to_owned(), if by_user { "conversation-user" } else { "conversation-agent" }.to_owned()),
    ];
    if has_image(body) {
        headers.push(("copilot-vision-request".into(), "true".into()));
    }
    if wire == Wire::Messages {
        headers.push(("anthropic-version".into(), "2023-06-01".into()));
    }
    headers
}

/// True when the messages after the model's last answer are the user's
/// own, not tool results going back to the model.
fn started_by_user(body: &Value) -> bool {
    let items = body["messages"].as_array().or_else(|| body["input"].as_array()).map(Vec::as_slice).unwrap_or_default();
    let mut user = false;
    for item in items.iter().rev() {
        let role = item["role"].as_str().unwrap_or_default();
        let kind = item["type"].as_str().unwrap_or_default();
        if role == "assistant" || kind == "function_call" || kind == "reasoning" {
            break;
        }
        let tool_result = role == "tool"
            || kind == "function_call_output"
            || item["content"].as_array().is_some_and(|blocks| blocks.iter().any(|b| b["type"] == "tool_result"));
        if tool_result {
            return false;
        }
        user |= role == "user";
    }
    user
}

fn has_image(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            matches!(map.get("type").and_then(Value::as_str), Some("image" | "image_url" | "input_image"))
                || map.values().any(has_image)
        }
        Value::Array(items) => items.iter().any(has_image),
        _ => false,
    }
}

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
    use super::*;
    use superfik_core::ModelProvider;
    use superfik_core::model::ModelCapabilities;

    fn served(endpoints: &[&str], caps: ModelCapabilities) -> ModelOption {
        let mut model = ModelOption::new("copilot-subscription:m", "M").served_by(ModelProvider::CopilotSubscription);
        model.capabilities =
            Some(ModelCapabilities { endpoints: endpoints.iter().map(|e| (*e).to_owned()).collect(), ..caps });
        model
    }

    fn header<'a>(headers: &'a [(String, String)], key: &str) -> Option<&'a str> {
        headers.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    #[test]
    fn each_model_is_served_on_the_best_api_it_offers() {
        let all = ["/chat/completions", "/responses", "/v1/messages"];
        assert_eq!(wire(Some(&served(&all, ModelCapabilities::default()))), Wire::Messages);
        assert_eq!(wire(Some(&served(&all[..2], ModelCapabilities::default()))), Wire::Responses);
        assert_eq!(wire(Some(&served(&all[..1], ModelCapabilities::default()))), Wire::Chat);
        assert_eq!(wire(None), Wire::Chat);
        assert_eq!(path(Wire::Messages), "/v1/messages");
    }

    #[test]
    fn calls_carry_the_copilot_cli_identity() {
        let sent = headers("gho_x");
        assert_eq!(header(&sent, "authorization"), Some("Bearer gho_x"));
        assert_eq!(header(&sent, "copilot-integration-id"), Some("copilot-developer-cli"));
        assert_eq!(header(&sent, "openai-intent"), Some("conversation-agent"));
        assert_eq!(header(&sent, "x-github-api-version"), Some("2026-06-01"));
        assert_eq!(header(&sent, "editor-version"), Some("copilot/1.0.62"));
    }

    #[test]
    fn only_a_turn_the_user_started_is_billed_as_theirs() {
        let user = json!({"messages": [{"role": "assistant", "content": "hi"}, {"role": "user", "content": "go"}]});
        assert_eq!(header(&turn_headers(Wire::Chat, &user), "x-initiator"), Some("user"));
        let tool = json!({"messages": [{"role": "assistant"}, {"role": "tool", "content": "ok"}, {"role": "user", "content": "note"}]});
        assert_eq!(header(&turn_headers(Wire::Chat, &tool), "x-initiator"), Some("agent"));
        let anthropic = json!({"messages": [{"role": "user", "content": [{"type": "tool_result"}]}]});
        assert_eq!(header(&turn_headers(Wire::Messages, &anthropic), "x-interaction-type"), Some("conversation-agent"));
        let responses = json!({"input": [{"type": "function_call"}, {"type": "function_call_output"}]});
        assert_eq!(header(&turn_headers(Wire::Responses, &responses), "x-initiator"), Some("agent"));
        let fresh = json!({"input": [{"type": "message", "role": "user", "content": [{"type": "input_image"}]}]});
        let sent = turn_headers(Wire::Responses, &fresh);
        assert_eq!(
            (header(&sent, "x-initiator"), header(&sent, "copilot-vision-request")),
            (Some("user"), Some("true"))
        );
    }

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
