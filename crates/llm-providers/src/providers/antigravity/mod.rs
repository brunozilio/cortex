pub mod auth;
pub mod models;
mod sign_in;
pub mod wire;

use domain::{Access, ModelOption};
use jiff::Timestamp;
use serde_json::Value;

use llm_providers::{Call, Config, Host, Page, Provider as Shape, ProviderError, TokenRequest, Tokens};

pub const BASE: &str = "https://daily-cloudcode-pa.googleapis.com";
pub const VERSION: &str = "2.24.0";

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "antigravity-subscription"
    }
    fn name(&self) -> &'static str {
        "Antigravity Subscription"
    }
    fn vendor(&self) -> &'static str {
        "antigravity"
    }
    fn access(&self) -> Access {
        Access::Browser
    }
    fn rank(&self) -> u8 {
        80
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0009
    }
    fn window(&self) -> u64 {
        200_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        wire::NAME
    }
    fn serves(&self, _model: &str) -> bool {
        true
    }
    fn check(&self, config: &Config) -> Result<(), ProviderError> {
        if config.refreshable()?.account_id.is_empty() {
            return Err(ProviderError::Unauthorized("the Antigravity project is missing; sign in again".into()));
        }
        Ok(())
    }
    fn base(&self, _config: &Config) -> String {
        BASE.to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        headers(config.access_token())
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/v1internal:streamGenerateContent?alt=sse"
    }
    fn bind(&self, config: &Config, body: &mut Value) {
        body["project"] = Value::String(config.account().to_owned());
    }
    fn retries_after_refresh(&self, status: u16) -> bool {
        status == 401
    }
    fn shape(&self, _model: Option<&ModelOption>, _body: &mut Value, _effort: &str) -> Result<(), ProviderError> {
        Ok(())
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        models::call(config)
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        Ok(Page { models: models::page(value), next: None })
    }
    fn finish(&self, models: Vec<ModelOption>) -> Vec<ModelOption> {
        if models.is_empty() { models::fallback() } else { models }
    }
    fn refresh(&self, previous: &Tokens) -> Option<TokenRequest> {
        Some(auth::refresh(previous))
    }
    fn tokens(
        &self,
        status: u16,
        body: &str,
        previous: Option<&Tokens>,
        now: Timestamp,
    ) -> Result<Tokens, ProviderError> {
        auth::tokens(status, body, previous, now)
    }
    fn sign_in(&self, host: &dyn Host) -> Result<Config, ProviderError> {
        let config = Config { auth: Some(sign_in::sign_in(host)?), ..Config::default() };
        config.validate(self)?;
        Ok(config)
    }
}

pub fn user_agent() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "amd64",
        other => other,
    };
    format!("antigravity/hub/{VERSION} (aidev_client; os_type={os}; arch={arch})")
}

pub fn headers(access_token: &str) -> Vec<(String, String)> {
    vec![
        ("authorization".into(), format!("Bearer {access_token}")),
        ("content-type".into(), "application/json".into()),
        ("accept".into(), "text/event-stream".into()),
        ("user-agent".into(), user_agent()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn signed_in() -> Config {
        let tokens = Tokens { refresh_token: "r".into(), account_id: "proj-1".into(), ..Tokens::bearer("a") };
        Config { auth: Some(tokens), ..Config::default() }
    }

    #[test]
    fn a_turn_goes_to_the_daily_host_for_the_accounts_project() {
        let config = signed_in();
        assert!(config.validate(&Provider).is_ok());
        assert_eq!(
            format!("{}{}", config.base(&Provider), Provider.path(None)),
            "https://daily-cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse"
        );
        let headers = config.headers(&Provider);
        assert!(headers.contains(&("authorization".into(), "Bearer a".into())));
        assert!(headers.iter().any(|(name, value)| name == "user-agent" && value.starts_with("antigravity/hub/")));
        let mut body = json!({"model": "antigravity-subscription:gemini-3.8-flash-high", "project": ""});
        llm_providers::request::shape(&Provider, None, &mut body).unwrap_or_else(|e| panic!("{e}"));
        Provider.bind(&config, &mut body);
        assert_eq!((body["model"].as_str(), body["project"].as_str()), (Some("gemini-3.8-flash-high"), Some("proj-1")));
    }

    #[test]
    fn a_credential_without_a_project_asks_for_a_new_sign_in() {
        let without =
            Config { auth: Some(Tokens { refresh_token: "r".into(), ..Tokens::bearer("a") }), ..Config::default() };
        assert!(matches!(without.validate(&Provider), Err(ProviderError::Unauthorized(_))));
    }

    #[test]
    fn an_empty_catalog_falls_back_to_the_known_models() {
        let models = llm_providers::catalog::collect(&Provider, |_| Ok::<_, ProviderError>(json!({"models": {}})))
            .unwrap_or_default();
        assert_eq!(models.len(), models::FALLBACK.len());
        assert!(Provider.serves("gemini-3.8-flash-high") && Provider.serves("claude-opus-5-5-high"));
    }
}
