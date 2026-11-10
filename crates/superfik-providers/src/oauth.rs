//! Signing in to the ChatGPT and Claude subscriptions: the clients, the
//! authorize address, the token requests and what their answers carry.
use super::ProviderError;
use base64::{
    Engine,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use superfik_core::ModelProvider;

pub const OPENAI_CLIENT: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CLAUDE_CLIENT: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
pub const OPENAI_REDIRECT: &str = "http://localhost:1455/auth/callback";
pub const OPENAI_TOKEN: &str = "https://auth.openai.com/oauth/token";
pub const CLAUDE_TOKEN: &str = "https://platform.claude.com/v1/oauth/token";
pub const OPENAI_CALLBACK: (u16, &str) = (1455, "/auth/callback");
pub const CLAUDE_CALLBACK: (u16, &str) = (0, "/callback");
pub const TOKEN_LIMIT: u64 = 1 << 20;

#[derive(Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens").field("expires_at", &self.expires_at).finish_non_exhaustive()
    }
}

impl Tokens {
    /// A token used as is, with no refresh token and no expiry (GitHub's).
    pub fn bearer(token: &str) -> Self {
        Self {
            access_token: token.to_owned(),
            refresh_token: String::new(),
            account_id: String::new(),
            expires_at: None,
        }
    }

    pub fn expired(&self) -> bool {
        self.expires_at.is_none_or(|at| Utc::now() + Duration::seconds(60) > at)
    }
}

pub fn subscription_client(client: Option<&str>) -> Option<ModelProvider> {
    match client {
        Some(OPENAI_CLIENT) => Some(ModelProvider::ChatGptSubscription),
        Some(CLAUDE_CLIENT) => Some(ModelProvider::ClaudeSubscription),
        _ => None,
    }
}

pub fn callback(provider: ModelProvider) -> (u16, &'static str) {
    if provider == ModelProvider::KiroSubscription {
        return super::kiro::auth::CALLBACK;
    }
    if provider == ModelProvider::ChatGptSubscription { OPENAI_CALLBACK } else { CLAUDE_CALLBACK }
}

pub fn state_bytes(provider: ModelProvider) -> usize {
    if provider == ModelProvider::ChatGptSubscription { 16 } else { 32 }
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(pairs.iter().copied()).finish()
}

pub fn authorize(provider: ModelProvider, challenge: &str, state: &str, redirect: &str) -> String {
    if provider == ModelProvider::KiroSubscription {
        return super::kiro::auth::portal(challenge, state, redirect);
    }
    if provider == ModelProvider::ChatGptSubscription {
        format!(
            "https://auth.openai.com/oauth/authorize?{}",
            form(&[
                ("response_type", "code"),
                ("client_id", OPENAI_CLIENT),
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
    } else {
        format!(
            "https://claude.com/cai/oauth/authorize?{}",
            form(&[
                ("code", "true"),
                ("client_id", CLAUDE_CLIENT),
                ("response_type", "code"),
                ("redirect_uri", redirect),
                ("scope", "user:inference"),
                ("code_challenge", challenge),
                ("code_challenge_method", "S256"),
                ("state", state),
            ])
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TokenRequest {
    pub url: &'static str,
    pub content_type: &'static str,
    pub body: String,
}

impl std::fmt::Debug for TokenRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRequest").field("url", &self.url).finish_non_exhaustive()
    }
}

/// The redirect a code is exchanged for, given the callback's query: the
/// loopback's own, except for Kiro, whose code is bound to the sign-in
/// the person picked on its portal.
pub fn exchange_redirect(provider: ModelProvider, redirect: &str, query: &str) -> Result<String, ProviderError> {
    if provider == ModelProvider::KiroSubscription {
        return super::kiro::auth::redirect(redirect, query);
    }
    Ok(redirect.to_owned())
}

pub fn exchange(provider: ModelProvider, code: &str, state: &str, verifier: &str, redirect: &str) -> TokenRequest {
    if provider == ModelProvider::KiroSubscription {
        return super::kiro::auth::exchange(code, verifier, redirect);
    }
    token_request(
        provider,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect),
            ("state", state),
        ],
    )
}

pub fn refresh(provider: ModelProvider, previous: &Tokens) -> TokenRequest {
    if provider == ModelProvider::KiroSubscription {
        return super::kiro::auth::refresh(previous);
    }
    token_request(provider, &[("grant_type", "refresh_token"), ("refresh_token", &previous.refresh_token)])
}

fn token_request(provider: ModelProvider, params: &[(&str, &str)]) -> TokenRequest {
    let openai = provider == ModelProvider::ChatGptSubscription;
    let mut params = params.to_vec();
    params.push(("client_id", if openai { OPENAI_CLIENT } else { CLAUDE_CLIENT }));
    if openai {
        params.retain(|(key, _)| *key != "state");
        TokenRequest { url: OPENAI_TOKEN, content_type: "application/x-www-form-urlencoded", body: form(&params) }
    } else {
        let body = Value::Object(params.iter().map(|(key, value)| ((*key).to_owned(), json!(value))).collect());
        TokenRequest { url: CLAUDE_TOKEN, content_type: "application/json", body: body.to_string() }
    }
}

pub fn tokens(
    provider: ModelProvider,
    status: u16,
    body: &str,
    previous: Option<&Tokens>,
    now: DateTime<Utc>,
) -> Result<Tokens, ProviderError> {
    if provider == ModelProvider::KiroSubscription {
        return super::kiro::auth::tokens(status, body, previous, now);
    }
    let unauthorized = |why: String| ProviderError::Unauthorized(why);
    if status != 200 {
        return Err(unauthorized(format!("token endpoint returned HTTP {status}")));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| unauthorized("invalid token response".into()))?;
    let access = value["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("token response has no access_token".into()))?;
    let account_id = if provider == ModelProvider::ChatGptSubscription {
        let id_token = value["id_token"].as_str().unwrap_or_default();
        account_id(id_token)
            .or_else(|_| account_id(access))
            .or_else(|error| previous.map(|p| p.account_id.clone()).filter(|id| !id.is_empty()).ok_or(error))?
    } else {
        String::new()
    };
    Ok(Tokens {
        access_token: access.to_owned(),
        refresh_token: value["refresh_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| previous.map(|p| p.refresh_token.clone()))
            .unwrap_or_default(),
        account_id,
        expires_at: Some(now + Duration::seconds(value["expires_in"].as_i64().unwrap_or(0))),
    })
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
mod tests {
    use super::*;

    fn jwt(claims: Value) -> String {
        format!("h.{}.s", URL_SAFE_NO_PAD.encode(claims.to_string()))
    }

    #[test]
    fn the_account_comes_from_any_of_the_three_claims() {
        let nested = jwt(json!({"https://api.openai.com/auth": {"chatgpt_account_id": "a1"}}));
        assert_eq!(account_id(&nested), Ok("a1".into()));
        assert_eq!(account_id(&jwt(json!({"chatgpt_account_id": "a2"}))), Ok("a2".into()));
        assert_eq!(account_id(&jwt(json!({"organizations": [{"id": "a3"}]}))), Ok("a3".into()));
        assert!(account_id("not-a-jwt").is_err());
    }

    #[test]
    fn openai_posts_a_form_without_state_and_claude_json_with_it() {
        let openai = exchange(ModelProvider::ChatGptSubscription, "c", "s", "v", OPENAI_REDIRECT);
        assert_eq!(openai.url, OPENAI_TOKEN);
        assert!(openai.body.contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann") && !openai.body.contains("state"));
        let claude = exchange(ModelProvider::ClaudeSubscription, "c", "s", "v", "http://localhost:9/callback");
        let body: Value = serde_json::from_str(&claude.body).unwrap_or_default();
        assert_eq!((body["state"].as_str(), body["client_id"].as_str()), (Some("s"), Some(CLAUDE_CLIENT)));
    }

    #[test]
    fn a_refresh_keeps_what_the_answer_omits() {
        let previous = Tokens {
            access_token: "old".into(),
            refresh_token: "r0".into(),
            account_id: "acct".into(),
            expires_at: None,
        };
        let now = Utc::now();
        let fresh = tokens(
            ModelProvider::ChatGptSubscription,
            200,
            r#"{"access_token":"new","expires_in":60}"#,
            Some(&previous),
            now,
        );
        let fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fresh.refresh_token.as_str(), fresh.account_id.as_str()), ("r0", "acct"));
        assert_eq!(fresh.expires_at, Some(now + Duration::seconds(60)));
        assert!(matches!(
            tokens(ModelProvider::ClaudeSubscription, 400, "{}", None, now),
            Err(ProviderError::Unauthorized(_))
        ));
    }

    #[test]
    fn each_client_id_names_its_subscription() {
        assert_eq!(subscription_client(Some(OPENAI_CLIENT)), Some(ModelProvider::ChatGptSubscription));
        assert_eq!(subscription_client(Some(CLAUDE_CLIENT)), Some(ModelProvider::ClaudeSubscription));
        assert_eq!(subscription_client(Some("other")), None);
    }
}
