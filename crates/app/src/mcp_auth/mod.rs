use std::sync::{Arc, Mutex};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use platform::sync::Locked;

use agent_engine::model::http::Http;
use domain::SecretId;
use domain::credentials::{McpSignIns, McpTokenSource};
use domain::mcp_oauth::{self, AuthServer, Authorize, McpTokens};
use serde_json::{Value, json};

use crate::providers::redirect::Loopback;
use crate::secrets::SecretStore;
use llm_providers::oauth::challenge;

const SLOT: SecretId = SecretId::from_uuid(uuid::Uuid::from_u128(0x6d63_705f_6f61_7574_6800_0000_0000_0001));
const FORM: &str = "application/x-www-form-urlencoded";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpAuthError {
    #[error("a sign-in could not be saved here ({0}), so the browser was not opened")]
    CannotKeep(String),
    #[error("{0} is not a safe address to sign in with (https, or http to this computer)")]
    UnsafeAddress(String),
    #[error("{0} is not a web address")]
    NotAWebAddress(String),
    #[error("{0} does not describe how to sign in (no OAuth metadata)")]
    NoMetadata(String),
    #[error(
        "this server does not let applications register themselves, so {name} cannot sign in. \
         Use a token the service issues instead, kept in the environment: \
         `{cmd} mcp add -H 'Authorization: Bearer ${{TOKEN}}' <name> {server}`",
        name = brand::NAME,
        cmd = brand::COMMAND,
    )]
    NoRegistration { server: String },
    #[error("{0}")]
    Failed(String),
}

impl From<String> for McpAuthError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

impl From<McpAuthError> for String {
    fn from(error: McpAuthError) -> Self {
        error.to_string()
    }
}

#[derive(Clone)]
pub struct McpAuth {
    secrets: Arc<dyn SecretStore>,
    http: Arc<dyn Http>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for McpAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpAuth")
    }
}

impl McpAuth {
    pub fn new(secrets: Arc<dyn SecretStore>, http: Arc<dyn Http>) -> Self {
        let clock = Arc::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        });
        Self { secrets, http, clock, lock: Arc::default() }
    }

    pub fn with_secrets(mut self, secrets: Arc<dyn SecretStore>) -> Self {
        self.secrets = secrets;
        self
    }

    #[cfg(test)]
    fn at(mut self, now: i64) -> Self {
        self.clock = Arc::new(move || now);
        self
    }

    pub fn for_run(&self) -> McpSignIns {
        McpSignIns(Arc::new(RunTokens { auth: self.clone() }))
    }

    pub fn sign_in(
        &self,
        server: &str,
        stop: &Arc<std::sync::atomic::AtomicBool>,
        open: &mut dyn FnMut(&str),
    ) -> Result<(), McpAuthError> {
        self.can_keep()?;
        let auth = self.auth_server(server)?;
        let loopback =
            Loopback::bind_on(0, "/callback", false).map_err(|e| e.to_string())?.stopped_by(Arc::clone(stop));
        let redirect = loopback.redirect_uri();
        let (client_id, client_secret) = self.register(&auth, server, &redirect)?;
        let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let state = uuid::Uuid::new_v4().simple().to_string();
        let challenge = challenge(&verifier);
        let url = Authorize {
            server: &auth,
            client_id: &client_id,
            redirect_uri: &redirect,
            challenge: &challenge,
            state: &state,
            resource: server,
        }
        .url();
        open(&url);
        let (code, _) = loopback.wait_callback(&state)?;
        let mut form = vec![
            ("grant_type", "authorization_code".to_owned()),
            ("code", code),
            ("redirect_uri", redirect),
            ("client_id", client_id.clone()),
            ("code_verifier", verifier),
            ("resource", server.to_owned()),
        ];
        if let Some(secret) = &client_secret {
            form.push(("client_secret", secret.clone()));
        }
        let answer = self.post_form(&auth.token_endpoint, &form)?;
        let blank = McpTokens {
            access_token: String::new(),
            refresh_token: None,
            expires_at: None,
            client_id,
            client_secret,
            token_endpoint: auth.token_endpoint,
            resource: server.to_owned(),
        };
        let tokens = blank.answered(&answer, (self.clock)())?;
        let _guard = self.lock.locked();
        Ok(self.save(server, Some(&tokens))?)
    }

    pub fn sign_out(&self, server: &str) -> Result<bool, McpAuthError> {
        let _guard = self.lock.locked();
        let had = self.load(server).is_some();
        self.save(server, None)?;
        Ok(had)
    }

    pub fn token(&self, server: &str) -> Option<String> {
        let tokens = {
            let _guard = self.lock.locked();
            self.load(server)?
        };
        let now = (self.clock)();
        if tokens.fresh(now) {
            return Some(tokens.access_token);
        }
        let _held = self
            .secrets
            .hold_for_refresh(refresh_slot(server))
            .inspect_err(|error| tracing::warn!(%error, server, "an MCP sign-in could not be refreshed"))
            .ok()?;
        let tokens = {
            let _guard = self.lock.locked();
            self.load(server)?
        };
        if tokens.fresh(now) {
            return Some(tokens.access_token);
        }
        let refresh = tokens.refresh_token.clone()?;
        let mut form = vec![
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", refresh),
            ("client_id", tokens.client_id.clone()),
            ("resource", tokens.resource.clone()),
        ];
        if let Some(secret) = &tokens.client_secret {
            form.push(("client_secret", secret.clone()));
        }
        let refreshed = self
            .post_form(&tokens.token_endpoint, &form)
            .and_then(|answer| tokens.answered(&answer, now))
            .inspect_err(|error| tracing::warn!(%error, server, "an MCP sign-in could not be refreshed"))
            .ok()?;
        let _guard = self.lock.locked();
        match self.load(server) {
            Some(kept) if kept.access_token != tokens.access_token && kept.fresh(now) => Some(kept.access_token),
            None => None,
            Some(_) => {
                if let Err(error) = self.save(server, Some(&refreshed)) {
                    tracing::warn!(%error, server, "a refreshed MCP sign-in could not be kept");
                }
                Some(refreshed.access_token)
            }
        }
    }

    fn can_keep(&self) -> Result<(), McpAuthError> {
        let _guard = self.lock.locked();
        self.secrets.update(SLOT, &mut |_| {}).map_err(McpAuthError::CannotKeep)
    }

    fn auth_server(&self, server: &str) -> Result<AuthServer, McpAuthError> {
        let refuse = |url: &str| McpAuthError::UnsafeAddress(url.to_owned());
        if !mcp_oauth::endpoint_ok(server) {
            return Err(refuse(server));
        }
        let resource = mcp_oauth::resource_metadata_urls(server).iter().find_map(|url| {
            let metadata = self.get_json(url)?;
            metadata["authorization_servers"][0].is_string().then_some(metadata)
        });
        let issuer = resource
            .as_ref()
            .and_then(|metadata| metadata["authorization_servers"][0].as_str().map(str::to_owned))
            .or_else(|| {
                server.split_once("://").map(|(scheme, rest)| {
                    format!("{scheme}://{}", rest.split(['/', '?', '#']).next().unwrap_or_default())
                })
            })
            .ok_or_else(|| McpAuthError::NotAWebAddress(server.to_owned()))?;
        if !mcp_oauth::endpoint_ok(&issuer) {
            return Err(refuse(&issuer));
        }
        let mut auth: AuthServer = mcp_oauth::auth_server_metadata_urls(&issuer)
            .iter()
            .find_map(|url| serde_json::from_value(self.get_json(url)?).ok())
            .ok_or_else(|| McpAuthError::NoMetadata(issuer.clone()))?;
        auth.resource_scopes = resource.as_ref().map(mcp_oauth::resource_scopes).unwrap_or_default();
        let endpoints =
            [Some(&auth.authorization_endpoint), Some(&auth.token_endpoint), auth.registration_endpoint.as_ref()];
        if let Some(unsafe_url) = endpoints.into_iter().flatten().find(|url| !mcp_oauth::endpoint_ok(url)) {
            return Err(refuse(unsafe_url));
        }
        Ok(auth)
    }

    fn register(
        &self,
        auth: &AuthServer,
        server: &str,
        redirect: &str,
    ) -> Result<(String, Option<String>), McpAuthError> {
        let endpoint = auth
            .registration_endpoint
            .as_deref()
            .ok_or_else(|| McpAuthError::NoRegistration { server: server.to_owned() })?;
        let body = json!({
            "client_name": brand::NAME,
            "redirect_uris": [redirect],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        });
        let headers = [("content-type".to_owned(), "application/json".to_owned())];
        let answer = self.read(self.http.post(endpoint, &headers, &body.to_string()))?;
        let id = answer["client_id"]
            .as_str()
            .ok_or_else(|| McpAuthError::Failed("the registration gave no client id".to_owned()))?;
        Ok((id.to_owned(), answer["client_secret"].as_str().map(str::to_owned)))
    }

    fn post_form(&self, url: &str, form: &[(&str, String)]) -> Result<Value, String> {
        let headers =
            [("content-type".to_owned(), FORM.to_owned()), ("accept".to_owned(), "application/json".to_owned())];
        self.read(self.http.post(url, &headers, &mcp_oauth::form(form)))
    }

    fn get_json(&self, url: &str) -> Option<Value> {
        self.read(self.http.get(url, &[("accept".to_owned(), "application/json".to_owned())])).ok()
    }

    fn read(&self, response: Result<agent_engine::model::http::Response, String>) -> Result<Value, String> {
        let response = response?;
        let status = response.status;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::Read::take(response.body, 1 << 20), &mut text)
            .map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return Ok(value);
        }
        let said = value["error_description"].as_str().or(value["error"].as_str()).unwrap_or_default();
        Err(format!("HTTP {status} {said}").trim_end().to_owned())
    }

    fn load(&self, server: &str) -> Option<McpTokens> {
        let kept = self.secrets.get(SLOT)?;
        serde_json::from_str(kept.get(server)?).ok()
    }

    fn save(&self, server: &str, tokens: Option<&McpTokens>) -> Result<(), String> {
        let json = tokens.map(serde_json::to_string).transpose().map_err(|e| e.to_string())?;
        self.secrets.update(SLOT, &mut |kept| match &json {
            Some(json) => {
                kept.insert(server.to_owned(), json.clone());
            }
            None => {
                kept.remove(server);
            }
        })
    }
}

fn refresh_slot(server: &str) -> SecretId {
    let digest = URL_SAFE_NO_PAD.decode(challenge(server)).unwrap_or_default();
    let mut bytes = [0u8; 16];
    for (slot, byte) in bytes.iter_mut().zip(digest) {
        *slot = byte;
    }
    SecretId::from_uuid(uuid::Uuid::from_bytes(bytes))
}

struct RunTokens {
    auth: McpAuth,
}

impl McpTokenSource for RunTokens {
    fn token(&self, server_url: &str) -> Option<String> {
        self.auth.token(server_url)
    }
}

pub(crate) mod command;

#[cfg(test)]
mod tests;
