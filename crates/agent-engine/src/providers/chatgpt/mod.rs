use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use domain::{Access, ModelOption};
use serde_json::{Value, json};

use llm_providers::catalog::{self, Call, Fields, Page};
use llm_providers::oauth::{self, Browser, TokenRequest, Tokens, form};
use llm_providers::request::remove;
use llm_providers::{Config, Host, Provider as Shape, ProviderError};

pub const CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const REDIRECT: &str = "http://localhost:1455/auth/callback";
pub const TOKEN: &str = "https://auth.openai.com/oauth/token";
pub const CALLBACK: (u16, &str) = (1455, "/auth/callback");

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "openai-subscription"
    }
    fn name(&self) -> &'static str {
        "ChatGPT Subscription"
    }
    fn vendor(&self) -> &'static str {
        "openai"
    }
    fn access(&self) -> Access {
        Access::Browser
    }
    fn rank(&self) -> u8 {
        40
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0004
    }
    fn window(&self) -> u64 {
        400_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        "responses"
    }
    fn check(&self, config: &Config) -> Result<(), ProviderError> {
        if config.refreshable()?.account_id.is_empty() {
            return Err(ProviderError::Unauthorized("ChatGPT account_id is missing".into()));
        }
        Ok(())
    }
    fn base(&self, _config: &Config) -> String {
        "https://chatgpt.com/backend-api".to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        let mut headers = vec![("accept".to_owned(), "text/event-stream".to_owned())];
        if let Some(tokens) = &config.auth {
            headers.extend([
                ("authorization".into(), format!("Bearer {}", tokens.access_token)),
                ("chatgpt-account-id".into(), tokens.account_id.clone()),
                ("openai-beta".into(), "responses=experimental".into()),
                ("originator".into(), "codex_cli_rs".into()),
            ]);
        }
        headers
    }
    fn turn_headers(&self, _model: Option<&ModelOption>, body: &Value) -> Vec<(String, String)> {
        body["prompt_cache_key"].as_str().map(|key| ("session_id".to_owned(), key.to_owned())).into_iter().collect()
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/codex/responses"
    }
    fn retries_after_refresh(&self, status: u16) -> bool {
        status == 401
    }
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
        body["instructions"] = json!(format!(
            "You are a helpful coding assistant operating through {}.\n\n{}",
            brand::NAME,
            body["instructions"].as_str().unwrap_or_default()
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
        llm_providers::effort(self.vendor(), body, model, effort, true)
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        Call::get(self, config, "/codex/models?client_version=0.0.0")
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let fields = Fields { id: "slug", window: "context_window" };
        let models = catalog::items(value, "models")?.iter().filter_map(|item| {
            let mut model = catalog::model(self, item, &fields)?;
            catalog::reasoning(&mut model, !catalog::levels(item).is_empty());
            Some(model)
        });
        Ok(Page { models: models.collect(), next: None })
    }
    fn refresh(&self, previous: &Tokens) -> Option<TokenRequest> {
        let pairs = [("grant_type", "refresh_token"), ("refresh_token", previous.refresh_token.as_str())];
        Some(request(&pairs))
    }
    fn tokens(
        &self,
        status: u16,
        body: &str,
        previous: Option<&Tokens>,
        now: DateTime<Utc>,
    ) -> Result<Tokens, ProviderError> {
        oauth::standard(status, body, previous, now, |value| {
            account_id(value["id_token"].as_str().unwrap_or_default())
                .or_else(|_| account_id(value["access_token"].as_str().unwrap_or_default()))
                .or_else(|error| previous.map(|p| p.account_id.clone()).filter(|id| !id.is_empty()).ok_or(error))
        })
    }
    fn sign_in(&self, host: &dyn Host) -> Result<Config, ProviderError> {
        let flow = Browser { callback: CALLBACK, state_bytes: 16, authorize: &authorize, exchange: &exchange };
        Ok(Config { auth: Some(oauth::browser(self, host, &flow)?), ..Config::default() })
    }
}

fn request(pairs: &[(&str, &str)]) -> TokenRequest {
    let mut pairs = pairs.to_vec();
    pairs.push(("client_id", CLIENT));
    TokenRequest::form(TOKEN, &pairs)
}

pub fn authorize(challenge: &str, state: &str, redirect: &str) -> String {
    format!(
        "https://auth.openai.com/oauth/authorize?{}",
        form(&[
            ("response_type", "code"),
            ("client_id", CLIENT),
            ("redirect_uri", redirect),
            ("scope", "openid profile email offline_access"),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("originator", "codex_cli_rs"),
        ])
    )
}

pub fn exchange(code: &str, _state: &str, verifier: &str, redirect: &str) -> TokenRequest {
    request(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("code_verifier", verifier),
        ("redirect_uri", redirect),
    ])
}

pub fn account_id(jwt: &str) -> Result<String, ProviderError> {
    let invalid = || ProviderError::Unauthorized("ChatGPT access token has no account id".into());
    let parts: Vec<_> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(invalid());
    }
    let data = URL_SAFE_NO_PAD.decode(parts[1]).or_else(|_| URL_SAFE.decode(parts[1])).map_err(|_| invalid())?;
    let value: Value = serde_json::from_slice(&data).map_err(|_| invalid())?;
    [
        &value["https://api.openai.com/auth"]["chatgpt_account_id"],
        &value["chatgpt_account_id"],
        &value["organizations"][0]["id"],
    ]
    .into_iter()
    .find_map(|claim| claim.as_str().filter(|s| !s.is_empty()))
    .map(str::to_owned)
    .ok_or_else(invalid)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
