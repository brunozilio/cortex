//! Signing in to Kiro through an organization's AWS IAM Identity Center, as
//! Kiro CLI does: the portal returns `/signin/callback?login_option=awsidc
//! &issuer_url=<start URL>&idc_region=<region>` (no code). This client then
//! registers with that region's SSO OIDC service, sends the browser to its
//! `/authorize` with PKCE, exchanges the code there, and finds the Kiro
//! profile the account may use with `ListAvailableProfiles`. Refreshes go
//! to the same OIDC service with the registered client.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::super::ProviderError;
use super::super::oauth::{TokenRequest, Tokens, form};

/// What a Kiro sign-in asks of the OIDC service.
pub const SCOPES: [&str; 3] = ["codewhisperer:completions", "codewhisperer:analysis", "codewhisperer:conversations"];
/// The services that list profiles, in the order they are asked.
pub const PROFILE_REGIONS: [&str; 2] = ["us-east-1", "eu-central-1"];
pub const PROFILES: &str = "AmazonCodeWhispererService.ListAvailableProfiles";
/// The name the organization's sign-in page shows: Kiro's, as this is a
/// Kiro sign-in.
pub const CLIENT_NAME: &str = "Kiro CLI";
/// Builder ID's start URL, for a portal answer that names none.
const BUILDER_ID: &str = "https://view.awsapps.com/start";
const LIFETIME: i64 = 3600;

/// The client registered with an Identity Center region: kept with the
/// tokens, since refreshing needs it.
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

/// How the person chose to sign in on Kiro's portal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalAnswer {
    /// Google or GitHub: the code is in the answer.
    Social,
    /// An Identity Center: its start URL and region.
    Organization { start_url: String, region: String },
}

/// Reads the portal's answer on `path` with `query`.
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

/// Whether `text` can be put in a host name as a region (`eu-west-1`).
fn region_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 32
        && text.contains('-')
        && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn oidc(region: &str) -> String {
    format!("https://oidc.{region}.amazonaws.com")
}

/// Registers this client with the region's OIDC service for `start_url`.
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

/// The client the OIDC service registered.
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

/// Where the browser signs in to the organization.
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

/// The OIDC service's token answer, with `client` kept for refreshes; the
/// profile ARN is the previous one (a sign-in finds it afterwards).
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
        idc: Some(client.clone()),
    })
}

/// The call that lists the profiles `access_token` may use in `region`:
/// its URL, headers and body.
pub fn profiles(region: &str, access_token: &str) -> (String, Vec<(String, String)>, String) {
    let headers = vec![
        ("authorization".into(), format!("Bearer {access_token}")),
        ("content-type".into(), "application/x-amz-json-1.0".into()),
        ("x-amz-target".into(), PROFILES.into()),
    ];
    (format!("https://q.{region}.amazonaws.com/"), headers, json!({ "maxResults": 10 }).to_string())
}

/// The first profile ARN a `ListAvailableProfiles` answer names.
pub fn first_profile(status: u16, body: &str) -> Option<String> {
    if status != 200 {
        return None;
    }
    let value: Value = serde_json::from_str(body).ok()?;
    value["profiles"].as_array()?.iter().find_map(|profile| profile["arn"].as_str().map(str::to_owned))
}

#[cfg(test)]
#[path = "idc_tests.rs"]
mod tests;
