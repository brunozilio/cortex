pub mod auth;
pub mod idc;
pub mod models;
mod sign_in;
pub mod wire;

use chrono::{DateTime, Utc};
use domain::{Access, ModelOption};
use serde_json::{Value, json};

use llm_providers::{Call, Config, Host, Page, Provider as Shape, ProviderError, TokenRequest, Tokens};

pub const ORIGIN: &str = "KIRO_CLI";
pub const CHAT: &str = "AmazonCodeWhispererStreamingService.GenerateAssistantResponse";
pub const MODELS: &str = "KiroControlPlaneBearerService.ListAvailableModels";
const DEFAULT_REGION: &str = "us-east-1";

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "kiro-subscription"
    }
    fn name(&self) -> &'static str {
        "Kiro Subscription"
    }
    fn vendor(&self) -> &'static str {
        "kiro"
    }
    fn small_model(&self) -> Option<&'static str> {
        Some("claude-haiku-4.5")
    }
    fn access(&self) -> Access {
        Access::Browser
    }
    fn rank(&self) -> u8 {
        60
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0007
    }
    fn window(&self) -> u64 {
        200_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        "event_stream"
    }
    fn check(&self, config: &Config) -> Result<(), ProviderError> {
        if config.refreshable()?.account_id.is_empty() {
            return Err(ProviderError::Unauthorized("Kiro profile ARN is missing".into()));
        }
        Ok(())
    }
    fn base(&self, config: &Config) -> String {
        runtime(config.account())
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        headers(config.access_token(), CHAT)
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/"
    }
    fn bind(&self, config: &Config, body: &mut Value) {
        if config.auth.is_some() {
            bind(body, config.account());
        }
    }
    fn retries_after_refresh(&self, status: u16) -> bool {
        matches!(status, 401 | 403)
    }
    fn shape(&self, _model: Option<&ModelOption>, body: &mut Value, _effort: &str) -> Result<(), ProviderError> {
        shape(body);
        Ok(())
    }
    fn catalog(&self, config: &Config, after: &str) -> Call {
        models::call(config, after)
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let (models, next) =
            models::page(value).ok_or_else(|| ProviderError::Invalid("model catalog has no model array".into()))?;
        Ok(Page { models, next })
    }
    fn refresh(&self, previous: &Tokens) -> Option<TokenRequest> {
        Some(match idc::client(previous) {
            Some(client) => idc::refresh(&client, previous),
            None => auth::refresh(previous),
        })
    }
    fn tokens(
        &self,
        status: u16,
        body: &str,
        previous: Option<&Tokens>,
        now: DateTime<Utc>,
    ) -> Result<Tokens, ProviderError> {
        match previous.and_then(idc::client) {
            Some(client) => idc::tokens(&client, status, body, previous, now),
            None => auth::tokens(status, body, previous, now),
        }
    }
    fn sign_in(&self, host: &dyn Host) -> Result<Config, ProviderError> {
        let config = Config { auth: Some(sign_in::sign_in(host)?), ..Config::default() };
        config.validate(self)?;
        Ok(config)
    }
}

pub fn region(profile_arn: &str) -> &str {
    profile_arn
        .split(':')
        .nth(3)
        .filter(|region| {
            !region.is_empty() && region.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
        .unwrap_or(DEFAULT_REGION)
}

pub fn runtime(profile_arn: &str) -> String {
    format!("https://runtime.{}.kiro.dev", region(profile_arn))
}

pub fn management(profile_arn: &str) -> String {
    format!("https://management.{}.kiro.dev", region(profile_arn))
}

pub fn headers(access_token: &str, target: &str) -> Vec<(String, String)> {
    vec![
        ("authorization".into(), format!("Bearer {access_token}")),
        ("content-type".into(), "application/x-amz-json-1.0".into()),
        ("x-amz-target".into(), target.into()),
    ]
}

pub fn shape(body: &mut Value) {
    let prefix = format!("{}:", Provider.id());
    let state = &mut body["conversationState"];
    let strip = |message: &mut Value| {
        if let Some(name) = message["userInputMessage"]["modelId"].as_str() {
            let bare = name.strip_prefix(&prefix).unwrap_or(name).to_owned();
            message["userInputMessage"]["modelId"] = json!(bare);
            message["userInputMessage"]["origin"] = json!(ORIGIN);
        }
    };
    strip(&mut state["currentMessage"]);
    if let Some(history) = state["history"].as_array_mut() {
        history.iter_mut().for_each(strip);
    }
}

pub fn bind(body: &mut Value, profile_arn: &str) {
    body["profileArn"] = json!(profile_arn);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARN: &str = "arn:aws:codewhisperer:eu-central-1:699475941385:profile/EHGA3GRVQMUK";

    #[test]
    fn the_profile_arn_picks_the_region_of_both_services() {
        assert_eq!(runtime(ARN), "https://runtime.eu-central-1.kiro.dev");
        assert_eq!(management(ARN), "https://management.eu-central-1.kiro.dev");
        assert_eq!(region(""), "us-east-1");
        assert_eq!(region("arn:aws:codewhisperer:evil.host/x:1:p"), "us-east-1");
    }

    #[test]
    fn a_turn_names_the_bare_model_and_the_account() {
        let mut body = json!({"conversationState": {
            "currentMessage": {"userInputMessage": {"modelId": "kiro-subscription:claude-sonnet-4.5"}},
            "history": [{"userInputMessage": {"modelId": "kiro-subscription:auto"}}, {"assistantResponseMessage": {}}]
        }});
        shape(&mut body);
        bind(&mut body, ARN);
        assert_eq!(body["conversationState"]["currentMessage"]["userInputMessage"]["modelId"], "claude-sonnet-4.5");
        assert_eq!(body["conversationState"]["history"][0]["userInputMessage"]["modelId"], "auto");
        assert_eq!(body["profileArn"], ARN);
        assert_eq!(body["conversationState"]["currentMessage"]["userInputMessage"]["origin"], ORIGIN);
    }

    #[test]
    fn it_is_served_from_the_profiles_region_with_its_token() {
        let tokens = Tokens { refresh_token: "r".into(), account_id: ARN.into(), ..Tokens::bearer("a") };
        let config = Config { auth: Some(tokens), ..Config::default() };
        assert!(config.validate(&Provider).is_ok());
        assert_eq!(config.base(&Provider), "https://runtime.eu-central-1.kiro.dev");
        let headers = config.headers(&Provider);
        assert!(headers.contains(&("authorization".into(), "Bearer a".into())));
        assert!(headers.contains(&("x-amz-target".into(), CHAT.into())));
        let without_arn =
            Config { auth: Some(Tokens { refresh_token: "r".into(), ..Tokens::bearer("a") }), ..Config::default() };
        assert!(without_arn.validate(&Provider).is_err());
    }
}

#[cfg(test)]
mod client_tests;
