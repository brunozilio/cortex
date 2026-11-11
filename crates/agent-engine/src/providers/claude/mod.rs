use chrono::{DateTime, Utc};
use domain::{Access, ModelOption};
use serde_json::{Value, json};

use llm_providers::catalog::{self, Call, Page, STANDARD};
use llm_providers::oauth::{self, Browser, TokenRequest, Tokens, form};
use llm_providers::request::remove;
use llm_providers::{Config, Host, Provider as Shape, ProviderError};

pub const CLIENT: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
pub const TOKEN: &str = "https://platform.claude.com/v1/oauth/token";
pub const CALLBACK: (u16, &str) = (0, "/callback");
pub const FALLBACK: [&str; 4] = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"];

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "claude-subscription"
    }
    fn name(&self) -> &'static str {
        "Claude Subscription"
    }
    fn vendor(&self) -> &'static str {
        "anthropic"
    }
    fn access(&self) -> Access {
        Access::Browser
    }
    fn rank(&self) -> u8 {
        20
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0005
    }
    fn window(&self) -> u64 {
        200_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        "messages"
    }
    fn check(&self, config: &Config) -> Result<(), ProviderError> {
        config.refreshable().map(|_| ())
    }
    fn base(&self, _config: &Config) -> String {
        "https://api.anthropic.com/v1".to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        let mut headers = vec![("accept".to_owned(), "text/event-stream".to_owned())];
        if let Some(tokens) = &config.auth {
            headers.extend([
                ("authorization".into(), format!("Bearer {}", tokens.access_token)),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("anthropic-beta".into(), "claude-code-20250219,oauth-2025-04-20".into()),
                ("anthropic-dangerous-direct-browser-access".into(), "true".into()),
                ("x-app".into(), "cli".into()),
                ("user-agent".into(), "claude-cli/2.1.207 (external, cli)".into()),
            ]);
        }
        headers
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/messages"
    }
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
        let mut blocks = vec![json!({"type":"text", "text":"You are Claude Code, Anthropic's official CLI for Claude."})];
        if let Some(existing) = body["system"].as_array() {
            blocks.extend(existing.iter().cloned());
        }
        body["system"] = json!(blocks);
        body["max_tokens"] = json!(8192);
        remove(body, &["thinking", "output_config", "cache_control"]);
        llm_providers::effort(self.vendor(), body, model, effort, false)
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        Call::get(self, config, "/models?limit=1000")
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let models = catalog::items(value, "data")?.iter().filter_map(|item| {
            let mut model = catalog::model(self, item, &STANDARD)?;
            catalog::reasoning(&mut model, true);
            Some(model)
        });
        Ok(Page { models: models.collect(), next: None })
    }
    fn finish(&self, mut models: Vec<ModelOption>) -> Vec<ModelOption> {
        if models.is_empty() {
            models.extend(FALLBACK.map(|id| catalog::option(self, id, id.to_owned())));
        }
        models
    }
    fn refresh(&self, previous: &Tokens) -> Option<TokenRequest> {
        Some(request(&[("grant_type", "refresh_token"), ("refresh_token", &previous.refresh_token)]))
    }
    fn tokens(
        &self,
        status: u16,
        body: &str,
        previous: Option<&Tokens>,
        now: DateTime<Utc>,
    ) -> Result<Tokens, ProviderError> {
        oauth::standard(status, body, previous, now, |_| Ok(String::new()))
    }
    fn sign_in(&self, host: &dyn Host) -> Result<Config, ProviderError> {
        let flow = Browser { callback: CALLBACK, state_bytes: 32, authorize: &authorize, exchange: &exchange };
        Ok(Config { auth: Some(oauth::browser(self, host, &flow)?), ..Config::default() })
    }
}

fn request(pairs: &[(&str, &str)]) -> TokenRequest {
    let mut pairs = pairs.to_vec();
    pairs.push(("client_id", CLIENT));
    TokenRequest::pairs_json(TOKEN, &pairs)
}

pub fn authorize(challenge: &str, state: &str, redirect: &str) -> String {
    format!(
        "https://claude.com/cai/oauth/authorize?{}",
        form(&[
            ("code", "true"),
            ("client_id", CLIENT),
            ("response_type", "code"),
            ("redirect_uri", redirect),
            ("scope", "user:inference"),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
        ])
    )
}

pub fn exchange(code: &str, state: &str, verifier: &str, redirect: &str) -> TokenRequest {
    request(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("code_verifier", verifier),
        ("redirect_uri", redirect),
        ("state", state),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_is_exchanged_as_json_with_the_state() {
        let request = exchange("c", "s", "v", "http://localhost:9/callback");
        let body: Value = serde_json::from_str(&request.body).unwrap_or_default();
        assert_eq!(request.url, TOKEN);
        assert_eq!((body["state"].as_str(), body["client_id"].as_str()), (Some("s"), Some(CLIENT)));
    }

    #[test]
    fn the_browser_is_sent_with_the_inference_scope() {
        let url = authorize("challenge", "state", "http://localhost:321/callback");
        assert!(url.starts_with("https://claude.com/cai/oauth/authorize?code=true&client_id="));
        assert!(url.contains("scope=user%3Ainference"));
    }

    #[test]
    fn it_speaks_as_claude_code() {
        let mut body = json!({"model": "claude-subscription:claude-sonnet-4-5", "system": [{"type": "text", "text": "x"}]});
        llm_providers::request::shape(&Provider, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(body["system"][0]["text"], "You are Claude Code, Anthropic's official CLI for Claude.");
        assert_eq!(body["max_tokens"], 8192);
    }

    #[test]
    fn an_empty_catalog_falls_back_to_the_current_priced_family() {
        let models = catalog::collect(&Provider, |_| Ok::<_, ProviderError>(json!({"data": []}))).unwrap_or_default();
        assert_eq!(models.len(), FALLBACK.len());
        assert!(catalog::collect(&Provider, |_| Ok::<_, ProviderError>(json!({}))).is_err());
        for id in FALLBACK {
            assert!(llm_providers::rates(Provider.vendor(), id, 0).is_some(), "{id}");
        }
    }

    #[test]
    fn it_needs_both_tokens() {
        let config = Config { auth: Some(Tokens::bearer("a")), ..Config::default() };
        assert!(config.validate(&Provider).is_err());
        let tokens = Tokens { refresh_token: "r".into(), ..Tokens::bearer("a") };
        assert!(Config { auth: Some(tokens), ..Config::default() }.validate(&Provider).is_ok());
    }
}
