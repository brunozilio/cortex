use llm_providers::ProviderError;
use llm_providers::oauth::{TokenRequest, Tokens, form};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};

pub const PORTAL: &str = "https://app.kiro.dev/signin";
pub const TOKEN: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/oauth/token";
pub const REFRESH: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken";
pub const CALLBACK: (u16, &str) = (49153, "/oauth/callback");
const LIFETIME: i64 = 3600;

pub fn portal(challenge: &str, state: &str, redirect: &str) -> String {
    let origin = redirect.strip_suffix(CALLBACK.1).unwrap_or(redirect);
    format!(
        "{PORTAL}?{}",
        form(&[
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("redirect_uri", origin),
            ("redirect_from", "kirocli"),
        ])
    )
}

pub fn redirect(callback: &str, query: &str) -> Result<String, ProviderError> {
    let option = url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == "login_option")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    match option.as_str() {
        "google" | "github" => Ok(format!("{callback}?{}", form(&[("login_option", &option)]))),
        "" => Err(ProviderError::Unauthorized("Kiro's sign-in did not say how you signed in".into())),
        other => Err(ProviderError::Unauthorized(format!(
            "signing in to Kiro with {other} is not supported yet; sign in with Google or GitHub"
        ))),
    }
}

pub fn exchange(code: &str, verifier: &str, redirect: &str) -> TokenRequest {
    let body = json!({"code": code, "code_verifier": verifier, "redirect_uri": redirect});
    TokenRequest { url: TOKEN.into(), content_type: "application/json", body: body.to_string() }
}

pub fn refresh(previous: &Tokens) -> TokenRequest {
    let body = json!({"refreshToken": previous.refresh_token});
    TokenRequest { url: REFRESH.into(), content_type: "application/json", body: body.to_string() }
}

pub fn tokens(status: u16, body: &str, previous: Option<&Tokens>, now: DateTime<Utc>) -> Result<Tokens, ProviderError> {
    let unauthorized = |why: String| ProviderError::Unauthorized(why);
    if status != 200 {
        return Err(unauthorized(format!("Kiro's auth service returned HTTP {status}")));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| unauthorized("invalid Kiro token response".into()))?;
    let text = |key: &str| value[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let access_token = text("accessToken").ok_or_else(|| unauthorized("Kiro returned no access token".into()))?;
    let kept = |key: &str, old: fn(&Tokens) -> &String| {
        text(key).or_else(|| previous.map(old).filter(|s| !s.is_empty()).cloned())
    };
    let refresh_token = kept("refreshToken", |t| &t.refresh_token)
        .ok_or_else(|| unauthorized("Kiro returned no refresh token".into()))?;
    let account_id =
        kept("profileArn", |t| &t.account_id).ok_or_else(|| unauthorized("Kiro returned no profile ARN".into()))?;
    let lifetime = value["expiresIn"].as_i64().filter(|s| *s > 0).unwrap_or(LIFETIME);
    Ok(Tokens {
        access_token,
        refresh_token,
        account_id,
        expires_at: Some(now + Duration::seconds(lifetime)),
        client: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARN: &str = "arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK";

    #[test]
    fn the_portal_gets_the_loopback_origin_and_the_challenge() {
        let url = portal("ch", "st", "http://localhost:49153/oauth/callback");
        assert_eq!(
            url,
            "https://app.kiro.dev/signin?state=st&code_challenge=ch&code_challenge_method=S256\
             &redirect_uri=http%3A%2F%2Flocalhost%3A49153&redirect_from=kirocli"
        );
    }

    #[test]
    fn the_code_is_exchanged_for_the_redirect_with_the_picked_sign_in() {
        let callback = "http://localhost:49153/oauth/callback";
        let redirect = redirect(callback, "login_option=google&code=c&state=s");
        assert_eq!(redirect, Ok("http://localhost:49153/oauth/callback?login_option=google".into()));
        let request = exchange("c", "v", "http://localhost:49153/oauth/callback?login_option=google");
        let body: Value = serde_json::from_str(&request.body).unwrap_or_default();
        assert_eq!(request.url, TOKEN);
        assert_eq!(
            body,
            json!({"code": "c", "code_verifier": "v",
                "redirect_uri": "http://localhost:49153/oauth/callback?login_option=google"})
        );
    }

    #[test]
    fn only_google_and_github_codes_go_to_kiros_auth_service() {
        let refused = super::redirect("http://x/oauth/callback", "login_option=awsIdc&code=c");
        assert!(matches!(refused, Err(ProviderError::Unauthorized(why)) if why.contains("awsIdc")));
        assert!(super::redirect("http://x/oauth/callback", "code=c").is_err());
    }

    #[test]
    fn an_answer_keeps_the_profile_arn_and_its_lifetime() {
        let now = Utc::now();
        let body = format!(r#"{{"accessToken":"a","refreshToken":"r","profileArn":"{ARN}","expiresIn":3600}}"#);
        let fresh = tokens(200, &body, None, now).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fresh.access_token.as_str(), fresh.refresh_token.as_str()), ("a", "r"));
        assert_eq!(fresh.account_id, ARN);
        assert_eq!(fresh.expires_at, Some(now + Duration::seconds(3600)));
    }

    #[test]
    fn a_refresh_keeps_what_the_answer_omits_and_failures_sign_out() {
        let previous = Tokens {
            access_token: "old".into(),
            refresh_token: "r0".into(),
            account_id: ARN.into(),
            expires_at: None,
            client: None,
        };
        assert_eq!(serde_json::from_str::<Value>(&refresh(&previous).body).ok(), Some(json!({"refreshToken": "r0"})));
        let fresh = tokens(200, r#"{"accessToken":"new"}"#, Some(&previous), Utc::now());
        let fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((fresh.refresh_token.as_str(), fresh.account_id.as_str()), ("r0", ARN));
        assert!(matches!(tokens(401, "{}", Some(&previous), Utc::now()), Err(ProviderError::Unauthorized(_))));
        assert!(tokens(200, r#"{"accessToken":"a","refreshToken":"r"}"#, None, Utc::now()).is_err(), "no ARN");
    }
}
