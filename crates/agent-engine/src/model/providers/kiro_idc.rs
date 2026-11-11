//! The HTTP calls of a Kiro sign-in through an organization's Identity
//! Center; what they send and read is in `llm_providers::kiro::idc`.

use llm_providers::kiro::idc::{self, IdcClient, PROFILE_REGIONS};
use llm_providers::oauth::{TOKEN_LIMIT, TokenRequest, Tokens};

use super::super::{ModelError, http::Http};

fn send(http: &dyn Http, request: &TokenRequest) -> Result<(u16, String), ModelError> {
    let headers = [("content-type".to_owned(), request.content_type.to_owned())];
    let response = http.post(&request.url, &headers, &request.body).map_err(ModelError::Network)?;
    let status = response.status;
    Ok((status, if status == 200 { super::transport::read(response, TOKEN_LIMIT)? } else { String::new() }))
}

/// Registers this client with the organization's region.
pub fn register(http: &dyn Http, region: &str, start_url: &str, redirect: &str) -> Result<IdcClient, ModelError> {
    let (status, body) = send(http, &idc::register(region, start_url, redirect))?;
    Ok(idc::registered(region, status, &body)?)
}

/// Exchanges the organization's code, then finds the Kiro profile the
/// account uses (the first one listed in us-east-1, then eu-central-1).
pub fn sign_in(
    http: &dyn Http,
    client: &IdcClient,
    code: &str,
    verifier: &str,
    redirect: &str,
) -> Result<Tokens, ModelError> {
    let (status, body) = send(http, &idc::exchange(client, code, verifier, redirect))?;
    let mut tokens = idc::tokens(client, status, &body, None, chrono::Utc::now())?;
    tokens.account_id = profile(http, &tokens.access_token)?;
    Ok(tokens)
}

fn profile(http: &dyn Http, access_token: &str) -> Result<String, ModelError> {
    for region in PROFILE_REGIONS {
        let (url, headers, body) = idc::profiles(region, access_token);
        let Ok(response) = http.post(&url, &headers, &body) else { continue };
        let status = response.status;
        let text = if status == 200 { super::transport::read(response, TOKEN_LIMIT)? } else { String::new() };
        if let Some(arn) = idc::first_profile(status, &text) {
            return Ok(arn);
        }
    }
    Err(ModelError::Unauthorized(
        "signed in, but this account has no Kiro profile; ask your organization's administrator to add you to one"
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::http::ScriptedHttp;

    const ARN: &str = "arn:aws:codewhisperer:eu-central-1:111122223333:profile/ABCDEF";

    fn client() -> IdcClient {
        IdcClient { region: "eu-west-1".into(), client_id: "cid".into(), client_secret: "secret".into() }
    }

    #[test]
    fn a_sign_in_registers_exchanges_the_code_and_finds_the_profile() {
        let http = ScriptedHttp::new([
            (200, r#"{"clientId":"cid","clientSecret":"secret"}"#.to_owned()),
            (200, r#"{"accessToken":"a","refreshToken":"r","expiresIn":3600}"#.to_owned()),
            (200, r#"{"profiles":[]}"#.to_owned()),
            (200, format!(r#"{{"profiles":[{{"arn":"{ARN}"}}]}}"#)),
        ]);
        let registered =
            register(&http, "eu-west-1", "https://acme.awsapps.com/start", "http://127.0.0.1:1/oauth/callback");
        assert_eq!(registered.expect("register"), client());
        let tokens =
            sign_in(&http, &client(), "code", "verifier", "http://127.0.0.1:1/oauth/callback").expect("tokens");
        assert_eq!((tokens.access_token.as_str(), tokens.account_id.as_str()), ("a", ARN));
        assert_eq!(tokens.idc, Some(client()));
        let urls: Vec<String> = http.sent().into_iter().map(|sent| sent.url).collect();
        assert_eq!(
            urls,
            [
                "https://oidc.eu-west-1.amazonaws.com/client/register",
                "https://oidc.eu-west-1.amazonaws.com/token",
                "https://q.us-east-1.amazonaws.com/",
                "https://q.eu-central-1.amazonaws.com/",
            ]
        );
    }

    #[test]
    fn an_account_without_a_profile_is_told_so() {
        let http = ScriptedHttp::new([
            (200, r#"{"accessToken":"a","refreshToken":"r"}"#.to_owned()),
            (403, String::new()),
            (200, r#"{"profiles":[]}"#.to_owned()),
        ]);
        let error = sign_in(&http, &client(), "code", "verifier", "http://x").expect_err("no profile");
        assert!(error.to_string().contains("no Kiro profile"), "{error}");
    }
}
