//! Browser sign-in for OAuth plugins, following the MCP authorization
//! flow: the server's protected-resource metadata names the authorization
//! server; its metadata gives the endpoints; the app registers itself
//! (dynamic client registration) unless the manifest names a client; the
//! user signs in through the browser (authorization code + PKCE, redirect
//! to a loopback port); tokens are refreshed before a run when they expire.

pub mod discovery;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use superfik_plugins::OAuthSpec;

use self::discovery::{Authorize, ServerMetadata, TokenResponse};
use super::secrets::Secrets;
use superfik_oauth::{Loopback, pkce};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthRequest {
    pub resource: String,
    pub spec: OAuthSpec,
}

pub trait OAuthFlow: Send + Sync {
    fn sign_in(&self, request: &OAuthRequest, open: &mut dyn FnMut(String)) -> Result<Secrets, String>;

    fn refresh(&self, secrets: &Secrets) -> Result<Option<Secrets>, String>;
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn expiring(secrets: &Secrets, now: u64) -> bool {
    let expires = secrets.get("expires_at").and_then(|value| value.parse::<u64>().ok());
    secrets.contains_key("refresh_token") && expires.is_some_and(|at| at <= now + 60)
}

pub fn secrets_from(
    token: TokenResponse,
    endpoint: &str,
    client: &Client,
    previous: Option<&Secrets>,
    now: u64,
) -> Secrets {
    let mut secrets = Secrets::new();
    secrets.insert("access_token".into(), token.access_token);
    let refresh = token.refresh_token.or_else(|| previous.and_then(|p| p.get("refresh_token").cloned()));
    if let Some(refresh) = refresh {
        secrets.insert("refresh_token".into(), refresh);
    }
    if let Some(seconds) = token.expires_in {
        secrets.insert("expires_at".into(), (now + seconds).to_string());
    }
    secrets.insert("token_endpoint".into(), endpoint.to_owned());
    secrets.insert("client_id".into(), client.id.clone());
    if let Some(secret) = &client.secret {
        secrets.insert("client_secret".into(), secret.clone());
    }
    secrets
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub id: String,
    pub secret: Option<String>,
}

#[derive(Debug, Clone)]
pub struct McpOAuth {
    agent: ureq::Agent,
}

impl Default for McpOAuth {
    fn default() -> Self {
        let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build();
        Self { agent: config.into() }
    }
}

impl McpOAuth {
    fn get_json(&self, url: &str) -> Result<Value, String> {
        let mut response =
            self.agent.get(url).header("Accept", "application/json").call().map_err(|e| e.to_string())?;
        response.body_mut().read_json::<Value>().map_err(|e| e.to_string())
    }

    fn post_form(&self, url: &str, body: &str) -> Result<TokenResponse, String> {
        let mut response = self
            .agent
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .send(body)
            .map_err(|e| e.to_string())?;
        response.body_mut().read_json::<TokenResponse>().map_err(|e| e.to_string())
    }

    fn issuer(&self, request: &OAuthRequest) -> Result<String, String> {
        if let Some(issuer) = &request.spec.authorization_server {
            return Ok(issuer.clone());
        }
        let fallback = discovery::split_origin(&request.resource).map(|(origin, _)| origin.to_owned());
        let named = discovery::protected_resource_url(&request.resource)
            .and_then(|url| self.get_json(&url).ok())
            .and_then(|metadata| discovery::authorization_server(&metadata));
        named.or(fallback).ok_or_else(|| format!("`{}` is not a URL", request.resource))
    }

    fn metadata(&self, issuer: &str) -> Result<ServerMetadata, String> {
        discovery::authorization_server_urls(issuer)
            .iter()
            .find_map(|url| serde_json::from_value(self.get_json(url).ok()?).ok())
            .ok_or_else(|| format!("{issuer} does not publish OAuth metadata"))
    }

    fn register(&self, metadata: &ServerMetadata, redirect_uri: &str) -> Result<Client, String> {
        let endpoint = metadata.registration_endpoint.as_deref().ok_or("this service needs a registered client id")?;
        let body = json!({
            "client_name": "Superfik",
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        });
        let mut response = self.agent.post(endpoint).send_json(&body).map_err(|e| e.to_string())?;
        let value = response.body_mut().read_json::<Value>().map_err(|e| e.to_string())?;
        let id = value["client_id"].as_str().ok_or("registration returned no client id")?.to_owned();
        Ok(Client { id, secret: value["client_secret"].as_str().map(str::to_owned) })
    }
}

impl OAuthFlow for McpOAuth {
    fn sign_in(&self, request: &OAuthRequest, open: &mut dyn FnMut(String)) -> Result<Secrets, String> {
        if let Some(provider) = super::model_providers::subscription_client(request.spec.client_id.as_deref()) {
            return super::model_providers::sign_in(provider, open);
        }
        let metadata = self.metadata(&self.issuer(request)?)?;
        let loopback = Loopback::bind()?;
        let redirect_uri = loopback.redirect_uri();
        let client = match &request.spec.client_id {
            Some(id) => Client { id: id.clone(), secret: None },
            None => self.register(&metadata, &redirect_uri)?,
        };
        let verifier = pkce::verifier();
        let state = uuid::Uuid::new_v4().simple().to_string();
        open(discovery::authorize_url(&Authorize {
            endpoint: &metadata.authorization_endpoint,
            client_id: &client.id,
            redirect_uri: &redirect_uri,
            challenge: &pkce::challenge(&verifier),
            state: &state,
            resource: &request.resource,
            scopes: &request.spec.scopes,
        }));
        let code = loopback.wait(&state)?;
        let mut params = vec![
            ("grant_type", "authorization_code".to_owned()),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client.id.clone()),
            ("code_verifier", verifier),
            ("resource", request.resource.clone()),
        ];
        if let Some(secret) = &client.secret {
            params.push(("client_secret", secret.clone()));
        }
        let token = self.post_form(&metadata.token_endpoint, &discovery::form(&params))?;
        Ok(secrets_from(token, &metadata.token_endpoint, &client, None, now()))
    }

    fn refresh(&self, secrets: &Secrets) -> Result<Option<Secrets>, String> {
        if !expiring(secrets, now()) {
            return Ok(None);
        }
        let field = |key: &str| secrets.get(key).cloned().ok_or_else(|| format!("missing {key}; sign in again"));
        let endpoint = field("token_endpoint")?;
        let client = Client { id: field("client_id")?, secret: secrets.get("client_secret").cloned() };
        let mut params = vec![
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", field("refresh_token")?),
            ("client_id", client.id.clone()),
        ];
        if let Some(secret) = &client.secret {
            params.push(("client_secret", secret.clone()));
        }
        let token = self.post_form(&endpoint, &discovery::form(&params))?;
        Ok(Some(secrets_from(token, &endpoint, &client, Some(secrets), now())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets(pairs: &[(&str, &str)]) -> Secrets {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn expiry_needs_a_refresh_token_and_a_near_deadline() {
        assert!(expiring(&secrets(&[("refresh_token", "r"), ("expires_at", "1030")]), 1000));
        assert!(!expiring(&secrets(&[("refresh_token", "r"), ("expires_at", "5000")]), 1000));
        assert!(!expiring(&secrets(&[("expires_at", "10")]), 1000), "nothing to refresh with");
        assert!(!expiring(&secrets(&[("refresh_token", "r")]), 1000), "no expiry known");
    }

    #[test]
    fn token_responses_keep_what_refreshing_needs() {
        let client = Client { id: "c".into(), secret: None };
        let token = TokenResponse { access_token: "a2".into(), refresh_token: None, expires_in: Some(3600) };
        let previous = secrets(&[("refresh_token", "r1")]);
        let kept = secrets_from(token, "https://t", &client, Some(&previous), 100);
        assert_eq!(kept["access_token"], "a2");
        assert_eq!(kept["refresh_token"], "r1", "an unrotated refresh token is kept");
        assert_eq!(kept["expires_at"], "3700");
        assert_eq!(kept["token_endpoint"], "https://t");
        assert_eq!(kept["client_id"], "c");
        assert!(!kept.contains_key("client_secret"));
    }
}
