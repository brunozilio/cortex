use std::time::Duration;

use serde_json::Value;

use super::auth;
use llm_providers::oauth::{self, Browser, Tokens};
use llm_providers::{Host, ProviderError};

const ONBOARD_ATTEMPTS: usize = 10;
const ONBOARD_WAIT: Duration = Duration::from_secs(5);

pub fn sign_in(host: &dyn Host) -> Result<Tokens, ProviderError> {
    let flow =
        Browser { callback: auth::CALLBACK, state_bytes: 32, authorize: &auth::authorize, exchange: &auth::exchange };
    let mut tokens = oauth::browser(&super::Provider, host, &flow)?;
    tokens.account_id = project(host, &tokens.access_token);
    if tokens.email.is_none() {
        tokens.email = host
            .get(auth::USERINFO, &[bearer(&tokens.access_token)])
            .ok()
            .filter(|(status, _)| *status == 200)
            .and_then(|(_, body)| auth::email(&body));
    }
    Ok(tokens)
}

fn bearer(access_token: &str) -> (String, String) {
    ("authorization".to_owned(), format!("Bearer {access_token}"))
}

fn call(host: &dyn Host, access_token: &str, method: &str, body: &str) -> Option<Value> {
    let url = format!("{}/v1internal:{method}", super::BASE);
    let mut headers = vec![bearer(access_token), ("content-type".to_owned(), "application/json".to_owned())];
    headers.push(("user-agent".to_owned(), super::user_agent()));
    let (status, text) = host.post(&url, &headers, body).ok()?;
    (status == 200).then(|| serde_json::from_str(&text).ok()).flatten()
}

fn project(host: &dyn Host, access_token: &str) -> String {
    let loaded = call(host, access_token, "loadCodeAssist", &auth::load());
    if let Some(found) = loaded.as_ref().and_then(auth::project) {
        return found;
    }
    let tier = loaded.as_ref().map_or_else(|| auth::FREE_TIER.to_owned(), auth::tier);
    let body = auth::onboard(&tier);
    for attempt in 0..ONBOARD_ATTEMPTS {
        let Some(answer) = call(host, access_token, "onboardUser", &body) else { break };
        if let Some(done) = auth::onboarded(&answer) {
            return done.unwrap_or_else(|| auth::DEFAULT_PROJECT.to_owned());
        }
        if attempt + 1 < ONBOARD_ATTEMPTS {
            host.sleep(ONBOARD_WAIT);
        }
    }
    auth::DEFAULT_PROJECT.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_providers::testing::ScriptedHost;

    const TOKENS: &str = r#"{"access_token":"a","refresh_token":"r","expires_in":3599}"#;

    #[test]
    fn a_sign_in_finds_the_accounts_project_and_email() {
        let host = ScriptedHost::new(
            [
                (200, TOKENS.to_owned()),
                (200, r#"{"cloudaicompanionProject":"proj-1","currentTier":{"id":"free-tier"}}"#.to_owned()),
                (200, r#"{"email":"me@example.com"}"#.to_owned()),
            ],
            [("/oauth-callback", "code=c&state=r1x32&scope=x")],
        );
        let tokens = sign_in(&host).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((tokens.access_token.as_str(), tokens.account_id.as_str()), ("a", "proj-1"));
        assert_eq!(tokens.email.as_deref(), Some("me@example.com"));
        let log = host.log.borrow();
        assert!(log.opened[0].0.starts_with(auth::AUTHORIZE), "{:?}", log.opened);
        let urls: Vec<_> = log.sent.iter().map(|(url, _)| url.as_str()).collect();
        assert_eq!(
            urls,
            [auth::TOKEN, "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist", auth::USERINFO]
        );
    }

    #[test]
    fn an_account_without_a_project_is_onboarded_until_done() {
        let host = ScriptedHost::new(
            [
                (200, TOKENS.to_owned()),
                (200, r#"{"allowedTiers":[{"id":"free-tier","isDefault":true}]}"#.to_owned()),
                (200, r#"{"done":false}"#.to_owned()),
                (200, r#"{"done":true,"response":{"cloudaicompanionProject":{"id":"proj-2"}}}"#.to_owned()),
                (404, String::new()),
            ],
            [("/oauth-callback", "code=c&state=r1x32")],
        );
        let tokens = sign_in(&host).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(tokens.account_id, "proj-2");
        assert_eq!(tokens.email, None);
        let log = host.log.borrow();
        assert!(log.sent[2].1.contains(r#""tier_id":"free-tier""#), "{}", log.sent[2].1);
        assert_eq!(log.waits, [ONBOARD_WAIT]);
    }

    #[test]
    fn without_any_project_the_shared_default_is_used() {
        let host = ScriptedHost::new(
            [(200, TOKENS.to_owned()), (500, String::new()), (500, String::new()), (500, String::new())],
            [("/oauth-callback", "code=c&state=r1x32")],
        );
        assert_eq!(sign_in(&host).map(|t| t.account_id), Ok(auth::DEFAULT_PROJECT.to_owned()));
    }

    #[test]
    fn a_refused_code_fails_the_sign_in() {
        let host = ScriptedHost::new(
            [(400, r#"{"error":"invalid_grant"}"#.to_owned())],
            [("/oauth-callback", "code=c&state=r1x32")],
        );
        assert!(matches!(sign_in(&host), Err(ProviderError::Unauthorized(_))));
        assert!(host.log.borrow().answered[0].starts_with("failed"));
    }
}
