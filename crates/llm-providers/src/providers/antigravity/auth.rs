use jiff::Timestamp;
use serde_json::{Value, json};

use llm_providers::ProviderError;
use llm_providers::oauth::{self, TokenRequest, Tokens, form};

pub const CLIENT: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
pub const SECRET: &str = "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";
pub const AUTHORIZE: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN: &str = "https://oauth2.googleapis.com/token";
pub const USERINFO: &str = "https://www.googleapis.com/oauth2/v1/userinfo?alt=json";
pub const CALLBACK: (u16, &str) = (51121, "/oauth-callback");
pub const SCOPES: [&str; 5] = [
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];
pub const DEFAULT_PROJECT: &str = "rising-fact-p41fc";
pub const FREE_TIER: &str = "free-tier";

pub fn authorize(challenge: &str, state: &str, redirect: &str) -> String {
    format!(
        "{AUTHORIZE}?{}",
        form(&[
            ("client_id", CLIENT),
            ("response_type", "code"),
            ("redirect_uri", redirect),
            ("scope", &SCOPES.join(" ")),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ])
    )
}

pub fn exchange(code: &str, _state: &str, verifier: &str, redirect: &str) -> TokenRequest {
    TokenRequest::form(
        TOKEN,
        &[
            ("client_id", CLIENT),
            ("client_secret", SECRET),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect),
            ("code_verifier", verifier),
        ],
    )
}

pub fn refresh(previous: &Tokens) -> TokenRequest {
    TokenRequest::form(
        TOKEN,
        &[
            ("client_id", CLIENT),
            ("client_secret", SECRET),
            ("grant_type", "refresh_token"),
            ("refresh_token", &previous.refresh_token),
        ],
    )
}

pub fn tokens(status: u16, body: &str, previous: Option<&Tokens>, now: Timestamp) -> Result<Tokens, ProviderError> {
    let mut tokens = oauth::standard(status, body, previous, now, |_| {
        Ok(previous.map(|previous| previous.account_id.clone()).unwrap_or_default())
    })?;
    if tokens.refresh_token.is_empty() {
        return Err(ProviderError::Unauthorized("Google returned no refresh token".into()));
    }
    if tokens.email.is_none() {
        tokens.email = previous.and_then(|previous| previous.email.clone());
    }
    Ok(tokens)
}

pub fn load() -> String {
    json!({"metadata": {"ideType": "ANTIGRAVITY"}}).to_string()
}

pub fn onboard(tier: &str) -> String {
    json!({"tier_id": tier, "metadata": {
        "ide_type": "ANTIGRAVITY",
        "ide_version": super::VERSION,
        "ide_name": "antigravity",
    }})
    .to_string()
}

pub fn project(value: &Value) -> Option<String> {
    let project = &value["cloudaicompanionProject"];
    project.as_str().or_else(|| project["id"].as_str()).map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned)
}

pub fn tier(value: &Value) -> String {
    let tiers = value["allowedTiers"].as_array().map(Vec::as_slice).unwrap_or_default();
    let id = |tier: &Value| tier["id"].as_str().filter(|id| !id.is_empty()).map(str::to_owned);
    tiers
        .iter()
        .find(|tier| tier["isDefault"] == true)
        .and_then(id)
        .or_else(|| tiers.first().and_then(id))
        .or_else(|| id(&value["currentTier"]))
        .unwrap_or_else(|| FREE_TIER.to_owned())
}

pub fn onboarded(value: &Value) -> Option<Option<String>> {
    (value["done"] == true).then(|| project(&value["response"]))
}

pub fn email(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value["email"].as_str().map(str::trim).filter(|email| email.contains('@') && email.len() <= 254).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_asks_google_for_offline_access_with_pkce() {
        let url = authorize("ch", "st", "http://localhost:51121/oauth-callback");
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?client_id=1071006060591-"));
        for wanted in [
            "redirect_uri=http%3A%2F%2Flocalhost%3A51121%2Foauth-callback",
            "scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform+",
            "code_challenge=ch&code_challenge_method=S256&state=st&access_type=offline&prompt=consent",
        ] {
            assert!(url.contains(wanted), "{wanted} in {url}");
        }
    }

    #[test]
    fn the_code_and_the_refresh_token_go_to_google_as_a_form() {
        let exchanged = exchange("c", "s", "v", "http://localhost:51121/oauth-callback");
        assert_eq!((exchanged.url.as_str(), exchanged.content_type), (TOKEN, "application/x-www-form-urlencoded"));
        assert!(exchanged.body.contains("grant_type=authorization_code") && exchanged.body.contains("code_verifier=v"));
        let previous = Tokens { refresh_token: "r0".into(), ..Tokens::bearer("a") };
        let refreshed = refresh(&previous);
        assert!(refreshed.body.contains("grant_type=refresh_token&refresh_token=r0"), "{}", refreshed.body);
        assert!(refreshed.body.contains(&format!("client_secret={SECRET}")));
    }

    #[test]
    fn a_refresh_keeps_the_project_the_refresh_token_and_the_email() {
        let previous = Tokens {
            refresh_token: "r0".into(),
            account_id: "proj-1".into(),
            email: Some("me@example.com".into()),
            ..Tokens::bearer("old")
        };
        let now = Timestamp::now();
        let fresh = tokens(200, r#"{"access_token":"new","expires_in":3599}"#, Some(&previous), now)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fresh.access_token.as_str(), fresh.refresh_token.as_str()), ("new", "r0"));
        assert_eq!(fresh.account_id, "proj-1");
        assert_eq!(fresh.email.as_deref(), Some("me@example.com"));
        assert_eq!(fresh.expires_at, Some(now + jiff::SignedDuration::from_secs(3599)));
        assert!(tokens(400, r#"{"error":"invalid_grant"}"#, Some(&previous), now).is_err());
        assert!(tokens(200, r#"{"access_token":"a"}"#, None, now).is_err(), "no refresh token");
    }

    #[test]
    fn the_project_is_read_as_a_name_or_an_object() {
        assert_eq!(project(&json!({"cloudaicompanionProject": "p-1"})).as_deref(), Some("p-1"));
        assert_eq!(project(&json!({"cloudaicompanionProject": {"id": "p-2"}})).as_deref(), Some("p-2"));
        assert_eq!(project(&json!({"cloudaicompanionProject": ""})), None);
        assert_eq!(project(&json!({})), None);
    }

    #[test]
    fn onboarding_takes_the_default_tier_and_waits_until_done() {
        let tiers = json!({"allowedTiers": [{"id": "legacy-tier"}, {"id": "standard-tier", "isDefault": true}]});
        assert_eq!(tier(&tiers), "standard-tier");
        assert_eq!(tier(&json!({"allowedTiers": [{"id": "legacy-tier"}]})), "legacy-tier");
        assert_eq!(tier(&json!({"currentTier": {"id": "g1-pro-tier"}})), "g1-pro-tier");
        assert_eq!(tier(&json!({})), FREE_TIER);
        let body: Value = serde_json::from_str(&onboard("free-tier")).unwrap_or_default();
        assert_eq!(body["tier_id"], "free-tier");
        assert_eq!(body["metadata"]["ide_type"], "ANTIGRAVITY");
        assert_eq!(onboarded(&json!({"done": false})), None);
        assert_eq!(
            onboarded(&json!({"done": true, "response": {"cloudaicompanionProject": {"id": "p"}}})),
            Some(Some("p".into()))
        );
    }

    #[test]
    fn the_email_comes_from_the_user_info() {
        assert_eq!(email(r#"{"email":"me@example.com","id":"1"}"#).as_deref(), Some("me@example.com"));
        assert_eq!(email(r#"{"id":"1"}"#), None);
    }
}
