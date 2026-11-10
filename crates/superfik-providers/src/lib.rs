//! The six LLM providers (OpenAI API, ChatGPT Subscription, Anthropic
//! API, Claude Subscription, GitHub Copilot Subscription, Kiro Subscription;
//! ADR 0016): what each one stores as its
//! credential, where it is served, how it signs in and how its requests and
//! catalogs are shaped. Pure: the native engine and the backend do the IO.

pub mod catalog;
pub mod copilot;
pub mod kiro;
pub mod oauth;
pub mod request;

use serde::{Deserialize, Serialize};
use superfik_core::ModelProvider;

pub use oauth::Tokens;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("not signed in: {0}")]
    Unauthorized(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub auth: Option<Tokens>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config").field("base_url", &self.base_url).finish_non_exhaustive()
    }
}

impl Config {
    pub fn parse(provider: ModelProvider, raw: &str) -> Result<Self, ProviderError> {
        let unauthorized = |why: &str| ProviderError::Unauthorized(why.to_owned());
        let config = if raw.trim_start().starts_with('{') {
            let mut value: serde_json::Value =
                serde_json::from_str(raw).map_err(|_| unauthorized("invalid provider credential JSON"))?;
            if let Some(raw_auth) = value["auth"].as_str() {
                value["auth"] =
                    serde_json::from_str(raw_auth).map_err(|_| unauthorized("invalid subscription token JSON"))?;
            }
            if value.get("access_token").is_some() {
                Self {
                    auth: Some(serde_json::from_value(value).map_err(|_| unauthorized("invalid subscription tokens"))?),
                    ..Self::default()
                }
            } else {
                serde_json::from_value(value).map_err(|_| unauthorized("invalid provider config"))?
            }
        } else if provider == ModelProvider::CopilotSubscription {
            Self { auth: Some(Tokens::bearer(raw.trim())), ..Self::default() }
        } else {
            Self { api_key: raw.trim().to_owned(), ..Self::default() }
        };
        config.validate(provider)?;
        Ok(config)
    }

    pub fn validate(&self, provider: ModelProvider) -> Result<(), ProviderError> {
        let invalid = |why: &str| ProviderError::Unauthorized(why.to_owned());
        if provider.subscription() {
            let tokens = self.auth.as_ref().ok_or_else(|| invalid("subscription credential is missing"))?;
            if provider == ModelProvider::CopilotSubscription {
                copilot_token(&tokens.access_token)?;
            } else if tokens.access_token.is_empty() || tokens.refresh_token.is_empty() {
                return Err(invalid("subscription needs access_token and refresh_token"));
            }
            if provider == ModelProvider::ChatGptSubscription && tokens.account_id.is_empty() {
                return Err(invalid("ChatGPT account_id is missing"));
            }
            if provider == ModelProvider::KiroSubscription && tokens.account_id.is_empty() {
                return Err(invalid("Kiro profile ARN is missing"));
            }
        } else if self.api_key.trim().is_empty() || self.api_key.chars().any(char::is_whitespace) {
            return Err(invalid("API key is empty or contains whitespace"));
        }
        if !self.base_url.is_empty() {
            let url = url::Url::parse(&self.base_url).map_err(|_| invalid("invalid API endpoint"))?;
            if !matches!(url.scheme(), "https" | "http")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(invalid("invalid API endpoint"));
            }
        }
        Ok(())
    }

    pub fn base(&self, provider: ModelProvider) -> String {
        if !self.base_url.is_empty() {
            return self.base_url.trim_end_matches('/').to_owned();
        }
        if provider == ModelProvider::KiroSubscription {
            return kiro::runtime(self.auth.as_ref().map_or("", |tokens| tokens.account_id.as_str()));
        }
        match provider {
            ModelProvider::OpenAi => "https://api.openai.com/v1",
            ModelProvider::ChatGptSubscription => "https://chatgpt.com/backend-api",
            ModelProvider::CopilotSubscription => copilot::API,
            _ => "https://api.anthropic.com/v1",
        }
        .to_owned()
    }

    pub fn headers(&self, provider: ModelProvider) -> Vec<(String, String)> {
        if provider == ModelProvider::KiroSubscription {
            return kiro::headers(self.auth.as_ref().map_or("", |tokens| tokens.access_token.as_str()), kiro::CHAT);
        }
        let mut headers: Vec<(String, String)> = vec![("accept".into(), "text/event-stream".into())];
        match provider {
            ModelProvider::ChatGptSubscription => {
                if let Some(tokens) = &self.auth {
                    headers.extend([
                        ("authorization".into(), format!("Bearer {}", tokens.access_token)),
                        ("chatgpt-account-id".into(), tokens.account_id.clone()),
                        ("openai-beta".into(), "responses=experimental".into()),
                        ("originator".into(), "codex_cli_rs".into()),
                    ]);
                }
            }
            ModelProvider::ClaudeSubscription => {
                if let Some(tokens) = &self.auth {
                    headers.extend([
                        ("authorization".into(), format!("Bearer {}", tokens.access_token)),
                        ("anthropic-version".into(), "2023-06-01".into()),
                        ("anthropic-beta".into(), "claude-code-20250219,oauth-2025-04-20".into()),
                        ("anthropic-dangerous-direct-browser-access".into(), "true".into()),
                        ("x-app".into(), "cli".into()),
                        ("user-agent".into(), "claude-cli/2.1.207 (external, cli)".into()),
                    ]);
                }
            }
            ModelProvider::CopilotSubscription => {
                if let Some(tokens) = &self.auth {
                    headers.extend(copilot::headers(&tokens.access_token));
                }
            }
            ModelProvider::OpenAi => headers.push(("authorization".into(), format!("Bearer {}", self.api_key))),
            _ => headers.extend([
                ("x-api-key".into(), self.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
            ]),
        }
        headers
    }

    pub fn needs_refresh(&self, provider: ModelProvider, force: bool) -> bool {
        provider.subscription()
            && provider != ModelProvider::CopilotSubscription
            && self.auth.as_ref().is_some_and(|t| force || t.expired())
    }
}

/// A GitHub token Copilot accepts: an OAuth or fine-grained one. Classic
/// personal access tokens are refused by Copilot, so they are here too.
fn copilot_token(token: &str) -> Result<(), ProviderError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(account: &str) -> Tokens {
        Tokens { access_token: "a".into(), refresh_token: "r".into(), account_id: account.into(), expires_at: None }
    }

    #[test]
    fn a_bare_key_is_an_api_key_and_whitespace_is_refused() {
        assert_eq!(Config::parse(ModelProvider::OpenAi, " sk-1 ").map(|c| c.api_key), Ok("sk-1".into()));
        assert!(Config::parse(ModelProvider::Anthropic, "sk ant").is_err());
    }

    #[test]
    fn chatgpt_needs_its_account_id() {
        let config = Config { auth: Some(tokens("")), ..Config::default() };
        assert!(config.validate(ModelProvider::ChatGptSubscription).is_err());
        assert!(config.validate(ModelProvider::ClaudeSubscription).is_ok());
    }

    #[test]
    fn copilot_takes_a_bare_github_token_but_not_a_classic_one() {
        let config = Config::parse(ModelProvider::CopilotSubscription, " gho_abc ").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(config.auth.as_ref().map(|t| t.access_token.as_str()), Some("gho_abc"));
        assert!(!config.needs_refresh(ModelProvider::CopilotSubscription, true), "GitHub tokens are not refreshed");
        assert!(Config::parse(ModelProvider::CopilotSubscription, "ghp_classic").is_err());
        assert!(Config::parse(ModelProvider::CopilotSubscription, "").is_err());
        let headers = config.headers(ModelProvider::CopilotSubscription);
        assert!(headers.contains(&("authorization".into(), "Bearer gho_abc".into())));
        assert_eq!(config.base(ModelProvider::CopilotSubscription), "https://api.githubcopilot.com");
    }

    #[test]
    fn kiro_is_served_from_its_profiles_region_with_its_token() {
        let arn = "arn:aws:codewhisperer:eu-central-1:1:profile/P";
        let config = Config { auth: Some(tokens(arn)), ..Config::default() };
        assert!(config.validate(ModelProvider::KiroSubscription).is_ok());
        assert_eq!(config.base(ModelProvider::KiroSubscription), "https://runtime.eu-central-1.kiro.dev");
        let headers = config.headers(ModelProvider::KiroSubscription);
        assert!(headers.contains(&("authorization".into(), "Bearer a".into())));
        assert!(headers.contains(&("x-amz-target".into(), kiro::CHAT.into())));
        let without_arn = Config { auth: Some(tokens("")), ..Config::default() };
        assert!(without_arn.validate(ModelProvider::KiroSubscription).is_err());
    }

    #[test]
    fn endpoints_with_credentials_or_queries_are_refused() {
        for url in ["ftp://x", "https://u:p@x", "https://x/?q", "https://x/#f", "nope"] {
            let config = Config { api_key: "k".into(), base_url: url.into(), auth: None };
            assert!(config.validate(ModelProvider::OpenAi).is_err(), "{url}");
        }
    }

    #[test]
    fn the_debug_form_hides_secrets() {
        let config = Config { api_key: "secret".into(), base_url: String::new(), auth: Some(tokens("acct")) };
        let shown = format!("{config:?}");
        assert!(!shown.contains("secret") && !shown.contains("acct"));
    }

    #[test]
    fn each_provider_is_served_from_its_own_base() {
        let config = Config::default();
        assert_eq!(config.base(ModelProvider::ChatGptSubscription), "https://chatgpt.com/backend-api");
        assert_eq!(config.base(ModelProvider::ClaudeSubscription), "https://api.anthropic.com/v1");
        let own = Config { base_url: "http://localhost:9/v1/".into(), ..Config::default() };
        assert_eq!(own.base(ModelProvider::OpenAi), "http://localhost:9/v1");
    }
}
