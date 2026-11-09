//! Members sign in to GitHub on an organization's server through its
//! GitHub App, with GitHub's device flow (ADR 0010): the server asks for a
//! code, the member types it at github.com, and the server keeps the
//! token, sealed, renewing it when it expires. Their GitHub commands then
//! act as them (`superfik_backend::GitHubTokens`).

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use superfik_backend::{GitHubTokens, SecretStore};
use superfik_core::{ConnectionId, Principal};

pub const GITHUB: ConnectionId =
    ConnectionId::from_uuid(uuid::Uuid::from_u128(0x6769_7468_7562_0000_0000_0000_0000_0001));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub url: String,
    pub interval: Duration,
    pub expires_in: Duration,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Granted {
    pub token: String,
    pub refresh: Option<String>,
    pub expires_unix: Option<u64>,
}

impl std::fmt::Debug for Granted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Granted").field("expires_unix", &self.expires_unix).finish_non_exhaustive()
    }
}

pub trait DeviceFlow: Send + Sync {
    fn begin(&self) -> Result<DeviceCode, String>;
    fn finish(&self, code: &DeviceCode, now: u64) -> Result<Granted, String>;
    fn refresh(&self, refresh: &str, now: u64) -> Result<Granted, String>;
    fn login(&self, token: &str) -> Result<String, String>;
}

#[derive(Clone)]
pub struct GitHubSignIn {
    pub tokens: Arc<KeptGitHubTokens>,
    pub app: Option<Arc<dyn DeviceFlow>>,
}

impl std::fmt::Debug for GitHubSignIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubSignIn").field("app", &self.app.is_some()).finish_non_exhaustive()
    }
}

impl GitHubSignIn {
    pub fn in_memory(app: Option<Arc<dyn DeviceFlow>>) -> Self {
        let secrets = Arc::new(superfik_backend::InMemorySecretStore::default());
        let now =
            Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()));
        let tokens = Arc::new(KeptGitHubTokens::new(secrets, app.clone(), now));
        Self { tokens, app }
    }
}

pub struct KeptGitHubTokens {
    secrets: Arc<dyn SecretStore>,
    flow: Option<Arc<dyn DeviceFlow>>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for KeptGitHubTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeptGitHubTokens").field("renews", &self.flow.is_some()).finish_non_exhaustive()
    }
}

impl KeptGitHubTokens {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        flow: Option<Arc<dyn DeviceFlow>>,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self { secrets, flow, now }
    }

    pub fn keep(&self, owner: &Principal, granted: &Granted) -> Result<(), String> {
        let json = serde_json::to_string(granted).map_err(|error| error.to_string())?;
        self.secrets.set(owner, GITHUB, &[("granted".to_owned(), json)].into_iter().collect())
    }

    pub fn forget(&self, owner: &Principal) {
        self.secrets.delete(owner, GITHUB);
    }

    fn kept(&self, owner: &Principal) -> Option<Granted> {
        let secrets = self.secrets.get(owner, GITHUB)?;
        serde_json::from_str(secrets.get("granted")?).ok()
    }
}

const MARGIN: u64 = 60;

impl GitHubTokens for KeptGitHubTokens {
    fn token(&self, owner: &Principal) -> Option<String> {
        let granted = self.kept(owner)?;
        let now = (self.now)();
        if granted.expires_unix.is_none_or(|expires| now + MARGIN < expires) {
            return Some(granted.token);
        }
        let renewed = self.flow.as_ref()?.refresh(granted.refresh.as_deref()?, now);
        match renewed {
            Ok(renewed) => {
                if let Err(error) = self.keep(owner, &renewed) {
                    tracing::warn!(%error, "could not keep a renewed GitHub token");
                }
                Some(renewed.token)
            }
            Err(error) => {
                tracing::warn!(%error, "a member's GitHub sign-in could not be renewed");
                None
            }
        }
    }
}

#[derive(Debug)]
pub struct GitHubApp {
    client_id: String,
}

impl GitHubApp {
    pub fn new(client_id: String) -> Self {
        Self { client_id }
    }

    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<serde_json::Value, String> {
        let mut response = ureq::post(url)
            .header("Accept", "application/json")
            .send_form(form.iter().copied())
            .map_err(|error| error.to_string())?;
        response.body_mut().read_json().map_err(|error| error.to_string())
    }
}

fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(serde_json::Value::as_str).map(str::to_owned)
}

fn granted(value: &serde_json::Value, now: u64) -> Option<Granted> {
    Some(Granted {
        token: text(value, "access_token")?,
        refresh: text(value, "refresh_token"),
        expires_unix: value.get("expires_in").and_then(serde_json::Value::as_u64).map(|seconds| now + seconds),
    })
}

impl DeviceFlow for GitHubApp {
    fn begin(&self) -> Result<DeviceCode, String> {
        let answer = self.post("https://github.com/login/device/code", &[("client_id", &self.client_id)])?;
        let seconds = |key: &str, default: u64| {
            Duration::from_secs(answer.get(key).and_then(serde_json::Value::as_u64).unwrap_or(default))
        };
        Ok(DeviceCode {
            device_code: text(&answer, "device_code").ok_or("GitHub gave no device code")?,
            user_code: text(&answer, "user_code").ok_or("GitHub gave no code")?,
            url: text(&answer, "verification_uri").unwrap_or_else(|| "https://github.com/login/device".to_owned()),
            interval: seconds("interval", 5),
            expires_in: seconds("expires_in", 900),
        })
    }

    fn finish(&self, code: &DeviceCode, now: u64) -> Result<Granted, String> {
        let mut interval = code.interval;
        let deadline = std::time::Instant::now() + code.expires_in;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(interval);
            let answer = self.post(
                "https://github.com/login/oauth/access_token",
                &[
                    ("client_id", &self.client_id),
                    ("device_code", &code.device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ],
            )?;
            if let Some(granted) = granted(&answer, now + interval.as_secs()) {
                return Ok(granted);
            }
            match text(&answer, "error").as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval += Duration::from_secs(5),
                Some(other) => return Err(format!("GitHub said {other}")),
                None => return Err("GitHub's answer was unreadable".to_owned()),
            }
        }
        Err("the code expired before it was typed".to_owned())
    }

    fn refresh(&self, refresh: &str, now: u64) -> Result<Granted, String> {
        let answer = self.post(
            "https://github.com/login/oauth/access_token",
            &[("client_id", &self.client_id), ("grant_type", "refresh_token"), ("refresh_token", refresh)],
        )?;
        granted(&answer, now).ok_or_else(|| text(&answer, "error").unwrap_or_else(|| "no token".to_owned()))
    }

    fn login(&self, token: &str) -> Result<String, String> {
        let mut response = ureq::get("https://api.github.com/user")
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "Superfik")
            .call()
            .map_err(|error| error.to_string())?;
        let user: serde_json::Value = response.body_mut().read_json().map_err(|error| error.to_string())?;
        text(&user, "login").ok_or_else(|| "GitHub named no user".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use superfik_backend::InMemorySecretStore;
    use superfik_core::{OrgId, UserId};

    use super::*;

    #[derive(Default)]
    struct Renewing(Mutex<u32>);

    impl DeviceFlow for Renewing {
        fn begin(&self) -> Result<DeviceCode, String> {
            Err("unused".into())
        }
        fn finish(&self, _: &DeviceCode, _: u64) -> Result<Granted, String> {
            Err("unused".into())
        }
        fn refresh(&self, refresh: &str, now: u64) -> Result<Granted, String> {
            *self.0.lock().expect("lock") += 1;
            (refresh == "r1")
                .then(|| Granted { token: "new".into(), refresh: Some("r2".into()), expires_unix: Some(now + 3600) })
                .ok_or_else(|| "bad refresh".into())
        }
        fn login(&self, _: &str) -> Result<String, String> {
            Ok("ana".into())
        }
    }

    #[test]
    fn a_kept_token_is_used_until_it_expires_then_renewed_once() {
        let clock = Arc::new(AtomicU64::new(1_000));
        let now = Arc::clone(&clock);
        let flow = Arc::new(Renewing::default());
        let tokens = KeptGitHubTokens::new(
            Arc::new(InMemorySecretStore::default()),
            Some(flow.clone()),
            Arc::new(move || now.load(Ordering::SeqCst)),
        );
        let ana = Principal::member(UserId::new(), OrgId::new());
        assert_eq!(tokens.token(&ana), None, "never signed in");
        let first = Granted { token: "old".into(), refresh: Some("r1".into()), expires_unix: Some(2_000) };
        tokens.keep(&ana, &first).expect("keep");
        assert_eq!(tokens.token(&ana).as_deref(), Some("old"));
        clock.store(1_990, Ordering::SeqCst);
        assert_eq!(tokens.token(&ana).as_deref(), Some("new"), "renewed before it lapses");
        assert_eq!(tokens.token(&ana).as_deref(), Some("new"), "and kept");
        assert_eq!(*flow.0.lock().expect("lock"), 1);
        tokens.forget(&ana);
        assert_eq!(tokens.token(&ana), None);
        assert!(!format!("{first:?}").contains("old"), "never in logs");
    }
}
