//! Signing in to remote MCP servers (`/mcp login <name>`): find the
//! server's authorization server, register Superfik with it, send the
//! person through the browser (PKCE, a loopback redirect), and keep the
//! tokens in the secrets store, by server URL. Runs get a fresh access token
//! through [`McpTokenSource`], refreshed when stale. The rules are
//! `domain::mcp_oauth`'s; this is their IO.

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use domain::credentials::{McpSignIns, McpTokenSource};
use domain::mcp_oauth::{self, AuthServer, Authorize, McpTokens};
use domain::{Principal, SecretId};
use agent_engine::model::http::Http;

use crate::providers::redirect::{Loopback, pkce};
use crate::secrets::SecretStore;

/// The secrets entry holding every server's tokens (URL → JSON).
const SLOT: SecretId = SecretId::from_uuid(uuid::Uuid::from_u128(0x6d63_705f_6f61_7574_6800_0000_0000_0001));
const FORM: &str = "application/x-www-form-urlencoded";

#[derive(Clone)]
pub struct McpAuth {
    secrets: Arc<dyn SecretStore>,
    http: Arc<dyn Http>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    /// One refresh or sign-in writes the tokens at a time.
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

    pub fn system() -> Self {
        Self::new(crate::secrets::system_secrets(), Arc::new(agent_engine::model::http::NetHttp))
    }

    #[cfg(test)]
    fn at(mut self, now: i64) -> Self {
        self.clock = Arc::new(move || now);
        self
    }

    /// What a run of `owner` reads its MCP tokens through.
    pub fn for_run(&self, owner: &Principal) -> McpSignIns {
        McpSignIns(Arc::new(RunTokens { auth: self.clone(), owner: owner.clone() }))
    }

    /// Signs `owner` in to the MCP server at `server`: `open` gets the
    /// address to visit in the browser; returns once signed in.
    pub fn sign_in(&self, owner: &Principal, server: &str, open: &mut dyn FnMut(&str)) -> Result<(), String> {
        let auth = self.auth_server(server)?;
        let loopback = Loopback::bind_on(0, "/callback", false)?;
        let redirect = loopback.redirect_uri();
        let (client_id, client_secret) = self.register(&auth, &redirect)?;
        let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
        let state = uuid::Uuid::new_v4().simple().to_string();
        let challenge = pkce::challenge(&verifier);
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
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.save(owner, server, Some(&tokens))
    }

    /// Forgets `owner`'s tokens for `server`; whether there were some.
    pub fn sign_out(&self, owner: &Principal, server: &str) -> Result<bool, String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let had = self.load(owner, server).is_some();
        self.save(owner, server, None)?;
        Ok(had)
    }

    /// A good access token for `server`, refreshed when stale.
    /// The refresh goes out without the lock, so one slow server holds up
    /// no other; a refresh that finished meanwhile wins.
    pub fn token(&self, owner: &Principal, server: &str) -> Option<String> {
        let tokens = {
            let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
            self.load(owner, server)?
        };
        let now = (self.clock)();
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
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        match self.load(owner, server) {
            Some(kept) if kept.access_token != tokens.access_token && kept.fresh(now) => Some(kept.access_token),
            // Signed out meanwhile: the refreshed token is not kept.
            None => None,
            Some(_) => {
                if let Err(error) = self.save(owner, server, Some(&refreshed)) {
                    tracing::warn!(%error, server, "a refreshed MCP sign-in could not be kept");
                }
                Some(refreshed.access_token)
            }
        }
    }

    /// The authorization server `server` names, or its own origin's.
    /// Every address it gives must pass [`mcp_oauth::endpoint_ok`].
    fn auth_server(&self, server: &str) -> Result<AuthServer, String> {
        let refuse =
            |url: &str| format!("{url} is not a safe address to sign in with (https, or http to this computer)");
        if !mcp_oauth::endpoint_ok(server) {
            return Err(refuse(server));
        }
        let issuer = mcp_oauth::resource_metadata_urls(server)
            .iter()
            .find_map(|url| self.get_json(url)?["authorization_servers"][0].as_str().map(str::to_owned))
            .or_else(|| {
                server.split_once("://").map(|(scheme, rest)| {
                    format!("{scheme}://{}", rest.split(['/', '?', '#']).next().unwrap_or_default())
                })
            })
            .ok_or_else(|| format!("{server} is not a web address"))?;
        if !mcp_oauth::endpoint_ok(&issuer) {
            return Err(refuse(&issuer));
        }
        let auth: AuthServer = mcp_oauth::auth_server_metadata_urls(&issuer)
            .iter()
            .find_map(|url| serde_json::from_value(self.get_json(url)?).ok())
            .ok_or_else(|| format!("{issuer} does not describe how to sign in (no OAuth metadata)"))?;
        let endpoints =
            [Some(&auth.authorization_endpoint), Some(&auth.token_endpoint), auth.registration_endpoint.as_ref()];
        if let Some(unsafe_url) = endpoints.into_iter().flatten().find(|url| !mcp_oauth::endpoint_ok(url)) {
            return Err(refuse(unsafe_url));
        }
        Ok(auth)
    }

    /// A client id for Superfik at `auth` (dynamic registration).
    fn register(&self, auth: &AuthServer, redirect: &str) -> Result<(String, Option<String>), String> {
        let endpoint = auth
            .registration_endpoint
            .as_deref()
            .ok_or("this server does not let applications register themselves, so Superfik cannot sign in")?;
        let body = json!({
            "client_name": "Superfik",
            "redirect_uris": [redirect],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        });
        let headers = [("content-type".to_owned(), "application/json".to_owned())];
        let answer = self.read(self.http.post(endpoint, &headers, &body.to_string()))?;
        let id = answer["client_id"].as_str().ok_or("the registration gave no client id")?;
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

    fn load(&self, owner: &Principal, server: &str) -> Option<McpTokens> {
        let kept = self.secrets.get(owner, SLOT)?;
        serde_json::from_str(kept.get(server)?).ok()
    }

    fn save(&self, owner: &Principal, server: &str, tokens: Option<&McpTokens>) -> Result<(), String> {
        let mut kept = self.secrets.get(owner, SLOT).unwrap_or_default();
        match tokens {
            Some(tokens) => {
                kept.insert(server.to_owned(), serde_json::to_string(tokens).map_err(|e| e.to_string())?);
            }
            None => {
                kept.remove(server);
            }
        }
        self.secrets.set(owner, SLOT, &kept)
    }
}

struct RunTokens {
    auth: McpAuth,
    owner: Principal,
}

impl McpTokenSource for RunTokens {
    fn token(&self, server_url: &str) -> Option<String> {
        self.auth.token(&self.owner, server_url)
    }
}

pub(crate) mod command;

#[cfg(test)]
mod tests;
