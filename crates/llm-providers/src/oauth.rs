use super::sign_in::{Host, post};
use super::{Provider, ProviderError};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const TOKEN_LIMIT: u64 = 1 << 20;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Value>,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens").field("expires_at", &self.expires_at).finish_non_exhaustive()
    }
}

impl Tokens {
    pub fn bearer(token: &str) -> Self {
        Self { access_token: token.to_owned(), ..Self::default() }
    }

    pub fn expired(&self) -> bool {
        self.expires_at.is_none_or(|at| Utc::now() + Duration::seconds(60) > at)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TokenRequest {
    pub url: String,
    pub content_type: &'static str,
    pub body: String,
}

impl std::fmt::Debug for TokenRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRequest").field("url", &self.url).finish_non_exhaustive()
    }
}

impl TokenRequest {
    pub fn json(url: impl Into<String>, body: &Value) -> Self {
        Self { url: url.into(), content_type: "application/json", body: body.to_string() }
    }

    pub fn form(url: impl Into<String>, pairs: &[(&str, &str)]) -> Self {
        Self { url: url.into(), content_type: "application/x-www-form-urlencoded", body: form(pairs) }
    }

    pub fn pairs_json(url: impl Into<String>, pairs: &[(&str, &str)]) -> Self {
        let body = Value::Object(pairs.iter().map(|(key, value)| ((*key).to_owned(), json!(value))).collect());
        Self::json(url, &body)
    }
}

pub fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(pairs.iter().copied()).finish()
}

pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn standard(
    status: u16,
    body: &str,
    previous: Option<&Tokens>,
    now: DateTime<Utc>,
    account: impl FnOnce(&Value) -> Result<String, ProviderError>,
) -> Result<Tokens, ProviderError> {
    let unauthorized = |why: String| ProviderError::Unauthorized(why);
    if status != 200 {
        return Err(unauthorized(format!("token endpoint returned HTTP {status}")));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| unauthorized("invalid token response".into()))?;
    let access = value["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("token response has no access_token".into()))?;
    Ok(Tokens {
        access_token: access.to_owned(),
        refresh_token: value["refresh_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| previous.map(|p| p.refresh_token.clone()))
            .unwrap_or_default(),
        account_id: account(&value)?,
        expires_at: Some(now + Duration::seconds(value["expires_in"].as_i64().unwrap_or(0))),
        client: None,
    })
}

pub struct Browser<'a> {
    pub callback: (u16, &'a str),
    pub state_bytes: usize,
    pub authorize: &'a dyn Fn(&str, &str, &str) -> String,
    pub exchange: &'a dyn Fn(&str, &str, &str, &str) -> TokenRequest,
}

impl std::fmt::Debug for Browser<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Browser").field("callback", &self.callback).finish_non_exhaustive()
    }
}

pub fn browser(provider: &dyn Provider, host: &dyn Host, flow: &Browser<'_>) -> Result<Tokens, ProviderError> {
    let listener = host.listen(flow.callback.0, flow.callback.1)?;
    let redirect = listener.redirect_uri();
    let verifier = host.random(64)?;
    let state = host.random(flow.state_bytes)?;
    host.open((flow.authorize)(&challenge(&verifier), &state, &redirect), None);
    let answer = listener.next(&state, &[])?;
    let code = answer.code();
    answer.finish(code.as_ref().map(|_| ()).map_err(ToString::to_string));
    let (status, body) = post(host, &(flow.exchange)(&code?, &state, &verifier, &redirect))?;
    provider.tokens(status, &body, None, host.now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_matches_the_rfc_example() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_refresh_keeps_the_refresh_token_the_answer_omits() {
        let previous = Tokens { refresh_token: "r0".into(), ..Tokens::bearer("old") };
        let now = Utc::now();
        let fresh =
            standard(200, r#"{"access_token":"new","expires_in":60}"#, Some(&previous), now, |_| Ok("a".into()));
        let fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fresh.refresh_token.as_str(), fresh.account_id.as_str()), ("r0", "a"));
        assert_eq!(fresh.expires_at, Some(now + Duration::seconds(60)));
        assert!(matches!(standard(400, "{}", None, now, |_| Ok(String::new())), Err(ProviderError::Unauthorized(_))));
    }
}
