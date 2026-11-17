mod body;
pub mod device;
pub mod models;
mod sign_in;

pub use body::shape;

use domain::{Access, ModelOption};
use serde_json::Value;

use llm_providers::catalog::{self, Call, Page};
use llm_providers::{Config, Host, Provider as Shape, ProviderError, Tokens};

pub const API: &str = "https://api.githubcopilot.com";
pub const VERSION: &str = "1.0.62";
pub const API_VERSION: &str = "2026-06-01";
pub const INTEGRATION: &str = "copilot-developer-cli";

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "copilot-subscription"
    }
    fn name(&self) -> &'static str {
        "GitHub Copilot Subscription"
    }
    fn vendor(&self) -> &'static str {
        "github"
    }
    fn access(&self) -> Access {
        Access::Code
    }
    fn rank(&self) -> u8 {
        50
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0006
    }
    fn window(&self) -> u64 {
        131_072
    }
    fn wire(&self, model: Option<&ModelOption>) -> &'static str {
        wire(model)
    }
    fn bare(&self, raw: &str) -> Config {
        Config { auth: Some(Tokens::bearer(raw.trim())), ..Config::default() }
    }
    fn check(&self, config: &Config) -> Result<(), ProviderError> {
        token(config.access_token())
    }
    fn base(&self, _config: &Config) -> String {
        API.to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        let mut sent = vec![("accept".to_owned(), "text/event-stream".to_owned())];
        if config.auth.is_some() {
            sent.extend(headers(config.access_token()));
        }
        sent
    }
    fn path(&self, model: Option<&ModelOption>) -> &'static str {
        path(wire(model))
    }
    fn turn_headers(&self, model: Option<&ModelOption>, body: &Value) -> Vec<(String, String)> {
        turn_headers(wire(model), body)
    }
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
        shape(model, body, effort)
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        Call::get(self, config, "/models")
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        Ok(Page { models: catalog::items(value, "data")?.iter().filter_map(models::model).collect(), next: None })
    }
    fn sign_in(&self, host: &dyn Host) -> Result<Config, ProviderError> {
        sign_in::sign_in(host)
    }
}

fn token(token: &str) -> Result<(), ProviderError> {
    let invalid = |why: &str| ProviderError::Unauthorized(why.to_owned());
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return Err(invalid("GitHub token is empty or contains whitespace"));
    }
    if token.starts_with("ghp_") {
        return Err(invalid(
            "classic personal access tokens (ghp_) do not work with Copilot; sign in, or use a fine-grained token with the Copilot Requests permission",
        ));
    }
    Ok(())
}

pub fn wire(model: Option<&ModelOption>) -> &'static str {
    let endpoints = model.and_then(|m| m.capabilities.as_ref()).map(|c| c.endpoints.as_slice()).unwrap_or_default();
    let offers = |path: &str| endpoints.iter().any(|e| e == path);
    if offers("/v1/messages") {
        "messages"
    } else if offers("/responses") {
        "responses"
    } else {
        "chat"
    }
}

pub fn path(wire: &str) -> &'static str {
    match wire {
        "messages" => "/v1/messages",
        "responses" => "/responses",
        "chat" => "/chat/completions",
        _ => "/",
    }
}

pub fn user_agent() -> String {
    format!("copilot/{VERSION} ({}) term/unknown", std::env::consts::OS)
}

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

pub fn turn_headers(wire: &str, body: &Value) -> Vec<(String, String)> {
    let by_user = started_by_user(body);
    let mut headers = vec![
        ("x-initiator".to_owned(), if by_user { "user" } else { "agent" }.to_owned()),
        ("x-interaction-type".to_owned(), if by_user { "conversation-user" } else { "conversation-agent" }.to_owned()),
    ];
    if has_image(body) {
        headers.push(("copilot-vision-request".into(), "true".into()));
    }
    if wire == "messages" {
        headers.push(("anthropic-version".into(), "2023-06-01".into()));
    }
    headers
}

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

#[cfg(test)]
pub(crate) fn served(endpoints: &[&str], caps: domain::model::ModelCapabilities) -> ModelOption {
    let mut model = ModelOption::new("copilot-subscription:m", "M").served_by(llm_providers::id(&Provider));
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
        assert_eq!(wire(Some(&served(&all, ModelCapabilities::default()))), "messages");
        assert_eq!(wire(Some(&served(&all[..2], ModelCapabilities::default()))), "responses");
        assert_eq!(wire(Some(&served(&all[..1], ModelCapabilities::default()))), "chat");
        assert_eq!(wire(None), "chat");
        assert_eq!(path("messages"), "/v1/messages");
    }

    #[test]
    fn a_bare_github_token_is_taken_but_not_a_classic_one() {
        let config = Config::parse(&Provider, " gho_abc ").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(config.access_token(), "gho_abc");
        assert!(!config.needs_refresh(&Provider, true), "GitHub tokens are not refreshed");
        assert!(Config::parse(&Provider, "ghp_classic").is_err());
        assert!(Config::parse(&Provider, "").is_err());
        assert!(config.headers(&Provider).contains(&("authorization".into(), "Bearer gho_abc".into())));
        assert_eq!(config.base(&Provider), "https://api.githubcopilot.com");
    }

    #[test]
    fn the_catalog_lists_its_data_page_once() {
        let page = json!({"data": [{"id": "claude-sonnet-4.5", "name": "Claude Sonnet 4.5"}, {"id": "x", "custom_model": true}]});
        let models = catalog::collect(&Provider, |_| Ok::<_, ProviderError>(page.clone()));
        assert_eq!(
            models.map(|m| m.into_iter().map(|m| m.id).collect::<Vec<_>>()),
            Ok(vec!["copilot-subscription:claude-sonnet-4.5".to_owned()])
        );
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
        assert_eq!(header(&turn_headers("chat", &user), "x-initiator"), Some("user"));
        let tool = json!({"messages": [{"role": "assistant"}, {"role": "tool", "content": "ok"}, {"role": "user", "content": "note"}]});
        assert_eq!(header(&turn_headers("chat", &tool), "x-initiator"), Some("agent"));
        let anthropic = json!({"messages": [{"role": "user", "content": [{"type": "tool_result"}]}]});
        assert_eq!(header(&turn_headers("messages", &anthropic), "x-interaction-type"), Some("conversation-agent"));
        let responses = json!({"input": [{"type": "function_call"}, {"type": "function_call_output"}]});
        assert_eq!(header(&turn_headers("responses", &responses), "x-initiator"), Some("agent"));
        let fresh = json!({"input": [{"type": "message", "role": "user", "content": [{"type": "input_image"}]}]});
        let sent = turn_headers("responses", &fresh);
        assert_eq!(
            (header(&sent, "x-initiator"), header(&sent, "copilot-vision-request")),
            (Some("user"), Some("true"))
        );
    }
}

#[cfg(test)]
mod client_tests;
