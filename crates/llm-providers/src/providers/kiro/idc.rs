use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ProviderError;
use crate::oauth::{TokenRequest, Tokens, form};

pub const SCOPES: [&str; 3] = ["codewhisperer:completions", "codewhisperer:analysis", "codewhisperer:conversations"];
pub const PROFILE_REGIONS: [&str; 2] = ["us-east-1", "eu-central-1"];
pub const PROFILES: &str = "AmazonCodeWhispererService.ListAvailableProfiles";
pub const CLIENT_NAME: &str = "Kiro CLI";
const BUILDER_ID: &str = "https://view.awsapps.com/start";
const LIFETIME: i64 = 3600;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdcClient {
    pub region: String,
    pub client_id: String,
    pub client_secret: String,
}

impl std::fmt::Debug for IdcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdcClient").field("region", &self.region).finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalAnswer {
    Social,
    Organization { start_url: String, region: String },
}

pub fn portal_answer(path: &str, query: &str) -> Result<PortalAnswer, ProviderError> {
    let value = |name: &str| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let refused = |why: String| Err(ProviderError::Unauthorized(why));
    let option = value("login_option").unwrap_or_default().to_ascii_lowercase();
    match option.as_str() {
        "google" | "github" if path == super::auth::CALLBACK.1 => Ok(PortalAnswer::Social),
        "awsidc" | "builderid" => {
            let builder = option == "builderid";
            let start_url = value("issuer_url").or_else(|| builder.then(|| BUILDER_ID.to_owned()));
            let region = value("idc_region").or_else(|| value("sso_region"));
            let region = region.or_else(|| builder.then(|| "us-east-1".to_owned()));
            let Some(start_url) = start_url else {
                return refused(
                    "Kiro's portal did not say which organization to sign in to (no issuer_url); \
                     check the start URL on the \"Your organization\" option and try again"
                        .into(),
                );
            };
            if !start_url.starts_with("https://") {
                return refused(format!("the organization's start URL is not https: {start_url}"));
            }
            match region {
                Some(region) if region_name(&region) => Ok(PortalAnswer::Organization { start_url, region }),
                Some(region) => refused(format!("`{region}` is not an AWS region")),
                None => refused("Kiro's portal did not say the organization's AWS region (no idc_region)".into()),
            }
        }
        "" => refused("Kiro's sign-in did not say how you signed in".into()),
        other => refused(format!("signing in to Kiro with {other} is not supported yet")),
    }
}

fn region_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 32
        && text.contains('-')
        && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn oidc(region: &str) -> String {
    format!("https://oidc.{region}.amazonaws.com")
}

pub fn register(region: &str, start_url: &str, redirect: &str) -> TokenRequest {
    let body = json!({
        "clientName": CLIENT_NAME,
        "clientType": "public",
        "scopes": SCOPES,
        "grantTypes": ["authorization_code", "refresh_token"],
        "redirectUris": [redirect],
        "issuerUrl": start_url,
    });
    TokenRequest {
        url: format!("{}/client/register", oidc(region)),
        content_type: "application/json",
        body: body.to_string(),
    }
}

pub fn registered(region: &str, status: u16, body: &str) -> Result<IdcClient, ProviderError> {
    let unauthorized = |why: String| ProviderError::Unauthorized(why);
    if status != 200 {
        return Err(unauthorized(format!("the organization's sign-in service returned HTTP {status}")));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| unauthorized("invalid client registration".into()))?;
    let text = |key: &str| value[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    match (text("clientId"), text("clientSecret")) {
        (Some(client_id), Some(client_secret)) => Ok(IdcClient { region: region.to_owned(), client_id, client_secret }),
        _ => Err(unauthorized("the organization's sign-in service registered no client".into())),
    }
}

pub fn authorize(client: &IdcClient, redirect: &str, state: &str, challenge: &str) -> String {
    let scopes = SCOPES.join(" ");
    format!(
        "{}/authorize?{}",
        oidc(&client.region),
        form(&[
            ("response_type", "code"),
            ("client_id", &client.client_id),
            ("redirect_uri", redirect),
            ("scopes", &scopes),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ])
    )
}

pub fn exchange(client: &IdcClient, code: &str, verifier: &str, redirect: &str) -> TokenRequest {
    let body = json!({
        "clientId": client.client_id,
        "clientSecret": client.client_secret,
        "grantType": "authorization_code",
        "code": code,
        "redirectUri": redirect,
        "codeVerifier": verifier,
    });
    TokenRequest {
        url: format!("{}/token", oidc(&client.region)),
        content_type: "application/json",
        body: body.to_string(),
    }
}

pub fn refresh(client: &IdcClient, previous: &Tokens) -> TokenRequest {
    let body = json!({
        "clientId": client.client_id,
        "clientSecret": client.client_secret,
        "grantType": "refresh_token",
        "refreshToken": previous.refresh_token,
    });
    TokenRequest {
        url: format!("{}/token", oidc(&client.region)),
        content_type: "application/json",
        body: body.to_string(),
    }
}

pub fn tokens(
    client: &IdcClient,
    status: u16,
    body: &str,
    previous: Option<&Tokens>,
    now: DateTime<Utc>,
) -> Result<Tokens, ProviderError> {
    let unauthorized = |why: String| ProviderError::Unauthorized(why);
    if status != 200 {
        return Err(unauthorized(format!("the organization's sign-in service returned HTTP {status}")));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| unauthorized("invalid token response".into()))?;
    let text = |key: &str| value[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let access_token =
        text("accessToken").ok_or_else(|| unauthorized("the sign-in returned no access token".into()))?;
    let refresh_token = text("refreshToken")
        .or_else(|| previous.map(|p| p.refresh_token.clone()).filter(|s| !s.is_empty()))
        .ok_or_else(|| unauthorized("the sign-in returned no refresh token".into()))?;
    let lifetime = value["expiresIn"].as_i64().filter(|s| *s > 0).unwrap_or(LIFETIME);
    Ok(Tokens {
        access_token,
        refresh_token,
        account_id: previous.map(|p| p.account_id.clone()).unwrap_or_default(),
        expires_at: Some(now + Duration::seconds(lifetime)),
        client: serde_json::to_value(client).ok(),
    })
}

pub fn profiles(region: &str, access_token: &str) -> (String, Vec<(String, String)>, String) {
    let headers = vec![
        ("authorization".into(), format!("Bearer {access_token}")),
        ("content-type".into(), "application/x-amz-json-1.0".into()),
        ("x-amz-target".into(), PROFILES.into()),
    ];
    (format!("https://q.{region}.amazonaws.com/"), headers, json!({ "maxResults": 10 }).to_string())
}

pub fn first_profile(status: u16, body: &str) -> Option<String> {
    if status != 200 {
        return None;
    }
    let value: Value = serde_json::from_str(body).ok()?;
    value["profiles"].as_array()?.iter().find_map(|profile| profile["arn"].as_str().map(str::to_owned))
}

pub fn client(tokens: &Tokens) -> Option<IdcClient> {
    tokens.client.clone().and_then(|value| serde_json::from_value(value).ok())
}

#[cfg(test)]
#[path = "idc_tests.rs"]
mod tests;
