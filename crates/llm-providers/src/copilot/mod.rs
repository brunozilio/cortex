//! The GitHub Copilot subscription as the Copilot CLI (`@github/copilot`
//! 1.0.62, checked against 1.0.108) speaks it: the GitHub token sent as is
//! to the Copilot API, each model served on the API its
//! `supported_endpoints` name, and the headers that bill a premium request
//! only for a turn the user started.
mod body;
pub mod device;
pub mod models;

pub use body::shape;

use serde_json::Value;
use domain::ModelOption;

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

/// A Copilot model offering `endpoints` with `caps`, for tests.
#[cfg(test)]
pub(crate) fn served(endpoints: &[&str], caps: domain::model::ModelCapabilities) -> ModelOption {
    let mut model =
        ModelOption::new("copilot-subscription:m", "M").served_by(domain::ModelProvider::CopilotSubscription);
    model.capabilities = Some(domain::model::ModelCapabilities {
        endpoints: endpoints.iter().map(|e| (*e).to_owned()).collect(),
        ..caps
    });
    model
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use domain::model::ModelCapabilities;

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
}
