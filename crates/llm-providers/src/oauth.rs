use super::sign_in::{Host, post};
use super::{Provider, ProviderError};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
    #[serde(default, serialize_with = "write_expiry", deserialize_with = "read_expiry")]
    pub expires_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
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
        self.expires_at.is_none_or(|at| at.duration_since(Timestamp::now()) < SignedDuration::from_secs(60))
    }
}

pub fn expiry(now: Timestamp, lifetime_secs: i64) -> Option<Timestamp> {
    now.checked_add(SignedDuration::from_secs(lifetime_secs)).ok()
}

fn rfc3339(at: Timestamp) -> String {
    let digits = match at.subsec_nanosecond() {
        0 => 0,
        nanos if nanos % 1_000_000 == 0 => 3,
        nanos if nanos % 1_000 == 0 => 6,
        _ => 9,
    };
    format!("{at:.digits$}")
}

fn write_expiry<S: Serializer>(at: &Option<Timestamp>, serializer: S) -> Result<S::Ok, S::Error> {
    match at {
        Some(at) => serializer.serialize_str(&rfc3339(*at)),
        None => serializer.serialize_none(),
    }
}

fn read_expiry<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Timestamp>, D::Error> {
    Option::<String>::deserialize(deserializer)?
        .map(|text| text.parse::<Timestamp>().map_err(serde::de::Error::custom))
        .transpose()
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
    now: Timestamp,
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
        expires_at: expiry(now, value["expires_in"].as_i64().unwrap_or(0)),
        client: None,
        email: account_label(body).or_else(|| previous.and_then(|p| p.email.clone())),
        subject: account_subject(body).or_else(|| previous.and_then(|p| p.subject.clone())),
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
    let exchanged = answer.code().and_then(|code| {
        let (status, body) = post(host, &(flow.exchange)(&code, &state, &verifier, &redirect))?;
        let tokens = provider.tokens(status, &body, None, host.now())?;
        Ok((tokens, account_label(&body)))
    });
    match exchanged {
        Ok((tokens, account)) => {
            answer.signed_in(account);
            Ok(tokens)
        }
        Err(error) => {
            answer.finish(Err(error.to_string()));
            Err(error)
        }
    }
}

pub fn account_label(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let [id, access] = token_claims(&value);
    [&value["account"]["email_address"], &value["account"]["email"], &value["email"], &id["email"], &access["email"]]
        .into_iter()
        .find_map(|found| found.as_str().map(str::trim).filter(|text| text.contains('@') && text.len() <= 254))
        .map(str::to_owned)
}

pub fn account_subject(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let [id, access] = token_claims(&value);
    [&id["sub"], &access["sub"]]
        .into_iter()
        .find_map(|found| found.as_str().map(str::trim).filter(|text| !text.is_empty() && text.len() <= 256))
        .map(str::to_owned)
}

fn token_claims(value: &Value) -> [Value; 2] {
    ["id_token", "access_token"].map(|key| value[key].as_str().and_then(jwt_claims).unwrap_or(Value::Null))
}

fn jwt_claims(jwt: &str) -> Option<Value> {
    let payload = jwt.split('.').nth(1)?;
    let data = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signed_in_account_is_read_from_the_token_answer_when_it_names_one() {
        let claims = URL_SAFE_NO_PAD.encode(r#"{"email":"dev@example.com"}"#);
        let id_token = format!(r#"{{"access_token":"a","id_token":"h.{claims}.s"}}"#);
        assert_eq!(account_label(&id_token).as_deref(), Some("dev@example.com"));
        let nested = r#"{"access_token":"a","account":{"email_address":"me@example.org"}}"#;
        assert_eq!(account_label(nested).as_deref(), Some("me@example.org"));
        assert_eq!(account_label(r#"{"access_token":"a","account_id":"acct_123"}"#), None);
        assert_eq!(account_label(r#"{"email":"not an address"}"#), None);
        assert_eq!(account_label("not json"), None);
    }

    #[test]
    fn the_expiry_is_written_to_disk_in_rfc_3339_utc_with_three_six_or_nine_fraction_digits() {
        let cases = [
            ("2026-11-05T03:30:00Z", "2026-11-05T03:30:00Z"),
            ("2026-11-05T03:30:00.000Z", "2026-11-05T03:30:00Z"),
            ("2026-11-05T03:30:00.1Z", "2026-11-05T03:30:00.100Z"),
            ("2026-11-05T03:30:00.120Z", "2026-11-05T03:30:00.120Z"),
            ("2026-11-05T03:30:00.1234Z", "2026-11-05T03:30:00.123400Z"),
            ("2026-11-05T03:30:00.123456Z", "2026-11-05T03:30:00.123456Z"),
            ("2026-11-05T03:30:00.0000005Z", "2026-11-05T03:30:00.000000500Z"),
            ("1999-12-31T23:59:59.999999999Z", "1999-12-31T23:59:59.999999999Z"),
            ("2026-11-05T05:30:00+02:00", "2026-11-05T03:30:00Z"),
            ("2026-11-04T22:30:00-05:00", "2026-11-05T03:30:00Z"),
            ("2099-01-01T00:00:00+00:00", "2099-01-01T00:00:00Z"),
        ];
        for (read, written) in cases {
            let tokens: Tokens = serde_json::from_value(json!({"access_token": "a", "expires_at": read}))
                .unwrap_or_else(|e| panic!("{read}: {e}"));
            let saved = serde_json::to_value(&tokens).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(saved["expires_at"], written, "{read}");
        }
    }

    #[test]
    fn saved_tokens_keep_their_field_order_and_an_unknown_expiry_is_null() {
        let tokens: Tokens =
            serde_json::from_str(r#"{"access_token":"a","expires_at":null}"#).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(tokens.expires_at, None);
        let saved = serde_json::to_string(&tokens).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(saved, r#"{"access_token":"a","refresh_token":"","account_id":"","expires_at":null}"#);
        let dated: Tokens = serde_json::from_str(r#"{"access_token":"a","expires_at":"2099-01-01T00:00:00Z"}"#)
            .unwrap_or_else(|e| panic!("{e}"));
        let saved = serde_json::to_string(&dated).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            saved,
            r#"{"access_token":"a","refresh_token":"","account_id":"","expires_at":"2099-01-01T00:00:00Z"}"#
        );
        assert!(serde_json::from_str::<Tokens>(r#"{"access_token":"a","expires_at":"tomorrow"}"#).is_err());
    }

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
        let now = Timestamp::now();
        let fresh =
            standard(200, r#"{"access_token":"new","expires_in":60}"#, Some(&previous), now, |_| Ok("a".into()));
        let fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fresh.refresh_token.as_str(), fresh.account_id.as_str()), ("r0", "a"));
        assert_eq!(fresh.expires_at, Some(now + SignedDuration::from_secs(60)));
        assert!(matches!(standard(400, "{}", None, now, |_| Ok(String::new())), Err(ProviderError::Unauthorized(_))));
    }

    #[test]
    fn without_an_email_the_token_s_subject_still_tells_accounts_apart() {
        let jwt = |claims: &str| format!("h.{}.s", URL_SAFE_NO_PAD.encode(claims));
        let from_id = format!(r#"{{"access_token":"a","id_token":"{}"}}"#, jwt(r#"{"sub":"user-1"}"#));
        assert_eq!(account_subject(&from_id).as_deref(), Some("user-1"));
        let from_access = format!(r#"{{"access_token":"{}"}}"#, jwt(r#"{"sub":"user-2","email":"two@x.ai"}"#));
        assert_eq!(account_subject(&from_access).as_deref(), Some("user-2"));
        assert_eq!(account_label(&from_access).as_deref(), Some("two@x.ai"));
        assert_eq!(account_subject(r#"{"access_token":"opaque"}"#), None);
        let now = Timestamp::now();
        let first = standard(200, &from_id, None, now, |_| Ok(String::new())).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(first.subject.as_deref(), Some("user-1"));
        let again = standard(200, r#"{"access_token":"b"}"#, Some(&first), now, |_| Ok(String::new()));
        assert_eq!(again.unwrap_or_else(|e| panic!("{e}")).subject.as_deref(), Some("user-1"));
    }

    #[test]
    fn the_account_s_email_is_kept_and_survives_a_refresh_that_omits_it() {
        let claims = URL_SAFE_NO_PAD.encode(r#"{"email":"dev@example.com"}"#);
        let answer = format!(r#"{{"access_token":"a","refresh_token":"r","id_token":"h.{claims}.s"}}"#);
        let now = Timestamp::now();
        let first = standard(200, &answer, None, now, |_| Ok(String::new())).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(first.email.as_deref(), Some("dev@example.com"));
        let again = standard(200, r#"{"access_token":"b"}"#, Some(&first), now, |_| Ok(String::new()));
        assert_eq!(again.unwrap_or_else(|e| panic!("{e}")).email.as_deref(), Some("dev@example.com"));
    }

    #[test]
    fn a_lifetime_past_the_calendar_leaves_the_expiry_unknown_instead_of_panicking() {
        let now = Timestamp::UNIX_EPOCH;
        let answer = format!(r#"{{"access_token":"a","expires_in":{}}}"#, i64::MAX);
        let tokens = standard(200, &answer, None, now, |_| Ok(String::new())).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(tokens.expires_at, None);
        assert!(tokens.expired(), "an unknown expiry is refreshed");
        assert_eq!(expiry(now, -1).map(rfc3339).as_deref(), Some("1969-12-31T23:59:59Z"));
    }

    #[test]
    fn a_token_counts_as_expired_a_minute_before_its_expiry() {
        let in_seconds = |secs: i64| Tokens { expires_at: expiry(Timestamp::now(), secs), ..Tokens::bearer("a") };
        assert!(in_seconds(30).expired());
        assert!(in_seconds(-30).expired());
        assert!(!in_seconds(120).expired());
    }
}
