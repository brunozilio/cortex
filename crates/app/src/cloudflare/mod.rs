use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_engine::model::http::Http;
use domain::SecretId;
use domain::mcp_oauth::{self, McpTokens};
use llm_providers::oauth::challenge;
use serde_json::Value;

use crate::providers::redirect::Loopback;
use crate::secrets::SecretStore;

const SLOT: SecretId = SecretId::from_uuid(uuid::Uuid::from_u128(0x636c_6f75_6466_6c61_7265_0000_0000_0001));
const KEPT: &str = "cloudflare";
const FORM: &str = "application/x-www-form-urlencoded";
pub const AUTHORIZE: &str = "https://dash.cloudflare.com/oauth2/auth";
pub const TOKEN: &str = "https://dash.cloudflare.com/oauth2/token";
pub const REVOKE: &str = "https://dash.cloudflare.com/oauth2/revoke";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudflareClient {
    pub id: String,
    pub port: u16,
    pub path: String,
    pub localhost: bool,
    pub scopes: String,
}

impl CloudflareClient {
    pub fn branded() -> Result<Self, String> {
        Self::parse(brand::cloudflare::CLIENT_ID, brand::cloudflare::REDIRECT, brand::cloudflare::SCOPES)
    }

    pub fn parse(id: &str, redirect: &str, scopes: &str) -> Result<Self, String> {
        let refuse = || format!("the Cloudflare redirect is http://localhost:<port>/<path> or 127.0.0.1: `{redirect}`");
        let rest = redirect.strip_prefix("http://").ok_or_else(refuse)?;
        let (host, path) = rest.split_once('/').ok_or_else(refuse)?;
        let (name, port) = host.split_once(':').ok_or_else(refuse)?;
        let localhost = match name {
            "localhost" => true,
            "127.0.0.1" => false,
            _ => return Err(refuse()),
        };
        let port = port.parse::<u16>().map_err(|_| refuse())?;
        if id.trim().is_empty() || scopes.trim().is_empty() {
            return Err("the Cloudflare client needs an id and scopes".into());
        }
        Ok(Self {
            id: id.trim().to_owned(),
            port,
            path: format!("/{path}"),
            localhost,
            scopes: scopes.trim().to_owned(),
        })
    }
}

pub struct CloudflareSignIn {
    secrets: Arc<dyn SecretStore>,
    http: Arc<dyn Http>,
    client: CloudflareClient,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl std::fmt::Debug for CloudflareSignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudflareSignIn").field("client", &self.client.id).finish_non_exhaustive()
    }
}

impl CloudflareSignIn {
    pub fn system() -> Result<Self, String> {
        Ok(Self::new(
            crate::secrets::system_secrets(),
            Arc::new(agent_engine::model::http::NetHttp),
            CloudflareClient::branded()?,
        ))
    }

    pub fn new(secrets: Arc<dyn SecretStore>, http: Arc<dyn Http>, client: CloudflareClient) -> Self {
        let clock = Arc::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        });
        Self { secrets, http, client, clock }
    }

    #[cfg(test)]
    fn at(mut self, now: i64) -> Self {
        self.clock = Arc::new(move || now);
        self
    }

    pub fn token(&self, stop: &Arc<AtomicBool>, open: &mut dyn FnMut(&str)) -> Result<String, String> {
        match self.kept() {
            Some(token) => Ok(token),
            None => self.sign_in(stop, open),
        }
    }

    pub fn kept(&self) -> Option<String> {
        let tokens = self.load()?;
        let now = (self.clock)();
        if tokens.fresh(now) {
            return Some(tokens.access_token);
        }
        let _held = self
            .secrets
            .hold_for_refresh(SLOT)
            .inspect_err(|error| tracing::warn!(%error, "the Cloudflare sign-in could not be refreshed"))
            .ok()?;
        let tokens = self.load()?;
        if tokens.fresh(now) {
            return Some(tokens.access_token);
        }
        let form = [
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", tokens.refresh_token.clone()?),
            ("client_id", self.client.id.clone()),
        ];
        let refreshed = self
            .post_form(TOKEN, &form)
            .and_then(|answer| tokens.answered(&answer, now))
            .inspect_err(|error| tracing::warn!(%error, "the Cloudflare sign-in could not be refreshed"))
            .ok()?;
        if let Err(error) = self.save(Some(&refreshed)) {
            tracing::warn!(%error, "a refreshed Cloudflare sign-in could not be kept");
        }
        Some(refreshed.access_token)
    }

    pub fn sign_in(&self, stop: &Arc<AtomicBool>, open: &mut dyn FnMut(&str)) -> Result<String, String> {
        self.secrets
            .update(SLOT, &mut |_| {})
            .map_err(|error| format!("a sign-in could not be saved here ({error}), so the browser was not opened"))?;
        let loopback = Loopback::bind_on(self.client.port, &self.client.path, self.client.localhost)
            .map_err(|error| error.to_string())?
            .stopped_by(Arc::clone(stop));
        let redirect = loopback.redirect_uri();
        let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let state = uuid::Uuid::new_v4().simple().to_string();
        let query = mcp_oauth::form(&[
            ("response_type", "code".to_owned()),
            ("client_id", self.client.id.clone()),
            ("redirect_uri", redirect.clone()),
            ("scope", self.client.scopes.clone()),
            ("state", state.clone()),
            ("code_challenge", challenge(&verifier)),
            ("code_challenge_method", "S256".to_owned()),
        ]);
        open(&format!("{AUTHORIZE}?{query}"));
        let (code, _) = loopback.wait_callback(&state)?;
        let answer = self.post_form(
            TOKEN,
            &[
                ("grant_type", "authorization_code".to_owned()),
                ("code", code),
                ("redirect_uri", redirect),
                ("client_id", self.client.id.clone()),
                ("code_verifier", verifier),
            ],
        )?;
        let blank = McpTokens {
            access_token: String::new(),
            refresh_token: None,
            expires_at: None,
            client_id: self.client.id.clone(),
            client_secret: None,
            token_endpoint: TOKEN.to_owned(),
            resource: String::new(),
        };
        let tokens = blank.answered(&answer, (self.clock)())?;
        self.save(Some(&tokens))?;
        Ok(tokens.access_token)
    }

    pub fn sign_out(&self) -> Result<bool, String> {
        let Some(tokens) = self.load() else { return Ok(false) };
        let token = tokens.refresh_token.unwrap_or(tokens.access_token);
        if let Err(error) = self.post_form(REVOKE, &[("token", token), ("client_id", self.client.id.clone())]) {
            tracing::warn!(%error, "Cloudflare did not confirm the sign-in was revoked");
        }
        self.save(None)?;
        Ok(true)
    }

    fn post_form(&self, url: &str, form: &[(&str, String)]) -> Result<Value, String> {
        let headers =
            [("content-type".to_owned(), FORM.to_owned()), ("accept".to_owned(), "application/json".to_owned())];
        let response = self.http.post(url, &headers, &mcp_oauth::form(form))?;
        let status = response.status;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::Read::take(response.body, 1 << 20), &mut text)
            .map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return Ok(value);
        }
        let said = value["error_description"].as_str().or(value["error"].as_str()).unwrap_or_default();
        Err(format!("Cloudflare answered HTTP {status} {said}").trim_end().to_owned())
    }

    fn load(&self) -> Option<McpTokens> {
        let kept = self.secrets.get(SLOT)?;
        let tokens: McpTokens = serde_json::from_str(kept.get(KEPT)?).ok()?;
        (tokens.client_id == self.client.id).then_some(tokens)
    }

    fn save(&self, tokens: Option<&McpTokens>) -> Result<(), String> {
        let json = tokens.map(serde_json::to_string).transpose().map_err(|error| error.to_string())?;
        self.secrets.update(SLOT, &mut |kept| match &json {
            Some(json) => {
                kept.insert(KEPT.to_owned(), json.clone());
            }
            None => {
                kept.remove(KEPT);
            }
        })
    }
}

#[cfg(test)]
mod tests;
