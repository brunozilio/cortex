use serde::{Deserialize, Serialize};

use super::{Provider, ProviderError, Tokens};

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

fn unauthorized(why: &str) -> ProviderError {
    ProviderError::Unauthorized(why.to_owned())
}

impl Config {
    pub fn parse(provider: &dyn Provider, raw: &str) -> Result<Self, ProviderError> {
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
        } else {
            provider.bare(raw)
        };
        config.validate(provider)?;
        Ok(config)
    }

    pub fn validate(&self, provider: &dyn Provider) -> Result<(), ProviderError> {
        if provider.access().subscription() {
            self.auth.as_ref().ok_or_else(|| unauthorized("subscription credential is missing"))?;
        } else if self.api_key.trim().is_empty() || self.api_key.chars().any(char::is_whitespace) {
            return Err(unauthorized("API key is empty or contains whitespace"));
        }
        provider.check(self)?;
        if !self.base_url.is_empty() {
            let url = url::Url::parse(&self.base_url).map_err(|_| unauthorized("invalid API endpoint"))?;
            if !matches!(url.scheme(), "https" | "http")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(unauthorized("invalid API endpoint"));
            }
        }
        Ok(())
    }

    pub fn refreshable(&self) -> Result<&Tokens, ProviderError> {
        let tokens = self.auth.as_ref().ok_or_else(|| unauthorized("subscription credential is missing"))?;
        if tokens.access_token.is_empty() || tokens.refresh_token.is_empty() {
            return Err(unauthorized("subscription needs access_token and refresh_token"));
        }
        Ok(tokens)
    }

    pub fn base(&self, provider: &dyn Provider) -> String {
        if self.base_url.is_empty() { provider.base(self) } else { self.base_url.trim_end_matches('/').to_owned() }
    }

    pub fn headers(&self, provider: &dyn Provider) -> Vec<(String, String)> {
        provider.headers(self)
    }

    pub fn needs_refresh(&self, provider: &dyn Provider, force: bool) -> bool {
        self.auth.as_ref().is_some_and(|tokens| (force || tokens.expired()) && provider.refresh(tokens).is_some())
    }

    pub fn access_token(&self) -> &str {
        self.auth.as_ref().map_or("", |tokens| tokens.access_token.as_str())
    }

    pub fn account(&self) -> &str {
        self.auth.as_ref().map_or("", |tokens| tokens.account_id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::KEYED;

    #[test]
    fn a_bare_key_is_an_api_key_and_whitespace_is_refused() {
        let keyed = &KEYED;
        assert_eq!(Config::parse(keyed, " sk-1 ").map(|c| c.api_key), Ok("sk-1".into()));
        assert!(Config::parse(keyed, "sk ant").is_err());
    }

    #[test]
    fn endpoints_with_credentials_or_queries_are_refused() {
        for url in ["ftp://x", "https://u:p@x", "https://x/?q", "https://x/#f", "nope"] {
            let config = Config { api_key: "k".into(), base_url: url.into(), auth: None };
            assert!(config.validate(&KEYED).is_err(), "{url}");
        }
    }

    #[test]
    fn an_own_endpoint_wins_without_its_trailing_slash() {
        let own = Config { base_url: "http://localhost:9/v1/".into(), ..Config::default() };
        assert_eq!(own.base(&KEYED), "http://localhost:9/v1");
    }

    #[test]
    fn the_debug_form_hides_secrets() {
        let tokens = Tokens { account_id: "acct".into(), ..Tokens::bearer("a") };
        let config = Config { api_key: "secret".into(), base_url: String::new(), auth: Some(tokens) };
        let shown = format!("{config:?}");
        assert!(!shown.contains("secret") && !shown.contains("acct"));
    }
}
