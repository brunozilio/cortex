use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::provider::ProviderKeyStore;
use domain::{ModelProvider, SecretId};
use llm_providers::sign_in::{Answer, Host, Listener};
use llm_providers::{Config, ProviderError};
use platform::sync::Locked;

use super::redirect::{Loopback, Pending};
use crate::secrets::{SecretStore, Secrets};

pub(crate) const CONFIG: &str = "config";
const ANSWER_LIMIT: u64 = 1 << 20;
const STOP_POLL: std::time::Duration = std::time::Duration::from_millis(100);
const REPLACED: &str = "this sign-in was replaced by a newer one";

pub trait SignIn: Send + Sync {
    fn sign_in(
        &self,
        provider: ModelProvider,
        stop: &Arc<AtomicBool>,
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<Config, ProviderError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BrowserSignIn;

impl SignIn for BrowserSignIn {
    fn sign_in(
        &self,
        provider: ModelProvider,
        stop: &Arc<AtomicBool>,
        open: &mut dyn FnMut(String, Option<String>),
    ) -> Result<Config, ProviderError> {
        let provider = agent_engine::providers::get(provider).map_err(|e| ProviderError::Invalid(e.to_string()))?;
        provider.sign_in(&NetHost { open: Mutex::new(open), stop: Arc::clone(stop) })
    }
}

struct NetHost<'a> {
    open: Mutex<&'a mut dyn FnMut(String, Option<String>)>,
    stop: Arc<AtomicBool>,
}

impl NetHost<'_> {
    fn going(&self) -> Result<(), ProviderError> {
        if self.stop.load(Ordering::SeqCst) {
            return Err(ProviderError::Invalid(REPLACED.to_owned()));
        }
        Ok(())
    }
}

fn read(response: agent_engine::model::http::Response) -> Result<(u16, String), ProviderError> {
    use std::io::Read;
    let status = response.status;
    let mut text = String::new();
    response
        .body
        .take(ANSWER_LIMIT)
        .read_to_string(&mut text)
        .map_err(|e| ProviderError::Invalid(format!("could not read the answer: {e}")))?;
    Ok((status, text))
}

impl Host for NetHost<'_> {
    fn post(&self, url: &str, headers: &[(String, String)], body: &str) -> Result<(u16, String), ProviderError> {
        use agent_engine::model::http::Http;
        self.going()?;
        read(agent_engine::model::http::NetHttp.post(url, headers, body).map_err(ProviderError::Invalid)?)
    }
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<(u16, String), ProviderError> {
        use agent_engine::model::http::Http;
        self.going()?;
        read(agent_engine::model::http::NetHttp.get(url, headers).map_err(ProviderError::Invalid)?)
    }
    fn open(&self, url: String, code: Option<String>) {
        let mut open = self.open.locked();
        open(url, code);
    }
    fn listen(&self, port: u16, path: &str) -> Result<Box<dyn Listener>, ProviderError> {
        Ok(Box::new(Loopback::bind_on(port, path, true)?.stopped_by(Arc::clone(&self.stop))))
    }
    fn random(&self, length: usize) -> Result<String, ProviderError> {
        let mut bytes = vec![0; length];
        getrandom::fill(&mut bytes).map_err(|e| ProviderError::Invalid(e.to_string()))?;
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }
    fn sleep(&self, duration: std::time::Duration) {
        let until = std::time::Instant::now() + duration;
        while !self.stop.load(Ordering::SeqCst) {
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return;
            }
            std::thread::sleep(left.min(STOP_POLL));
        }
    }
    fn now(&self) -> jiff::Timestamp {
        jiff::Timestamp::now()
    }
}

impl Listener for Loopback {
    fn redirect_uri(&self) -> String {
        Loopback::redirect_uri(self)
    }
    fn port(&self) -> u16 {
        Loopback::port(self)
    }
    fn next(&self, state: &str, others: &[&str]) -> Result<Box<dyn Answer>, ProviderError> {
        Ok(Box::new(Loopback::next(self, state, others).map_err(ProviderError::Unauthorized)?))
    }
}

impl Answer for Pending {
    fn path(&self) -> &str {
        &self.path
    }
    fn query(&self) -> &str {
        &self.query
    }
    fn code(&self) -> Result<String, ProviderError> {
        Pending::code(self).map_err(ProviderError::Unauthorized)
    }
    fn redirect(self: Box<Self>, to: &str) {
        Pending::redirect(*self, to);
    }
    fn finish(self: Box<Self>, result: Result<(), String>) {
        Pending::finish(*self, result.as_ref().map(|_| ()));
    }
    fn signed_in(self: Box<Self>, account: Option<String>) {
        Pending::signed_in(*self, account.as_deref());
    }
}

pub(crate) struct AccountStore {
    pub secrets: Arc<dyn SecretStore>,
    pub slot: SecretId,
    pub provider: ModelProvider,
}

impl ProviderKeyStore for AccountStore {
    fn load(&self) -> Result<String, String> {
        let values = self.secrets.get(self.slot).ok_or("provider account disconnected")?;
        values.get(CONFIG).cloned().ok_or_else(|| "provider account has no credential".to_owned())
    }

    fn save(&self, value: &str) -> Result<(), String> {
        let provider = agent_engine::providers::get(self.provider).map_err(|e| e.to_string())?;
        Config::parse(provider, value).map_err(|e| e.to_string())?;
        let mut secrets = self.secrets.get(self.slot).ok_or("provider account disconnected")?;
        secrets.insert(CONFIG.into(), value.to_owned());
        self.secrets.set(self.slot, &secrets)
    }

    fn hold_for_refresh(&self) -> Result<Option<domain::provider::RefreshHold>, String> {
        self.secrets.hold_for_refresh(self.slot)
    }
}

pub(crate) fn secrets(config: &Config) -> Result<Secrets, String> {
    Ok(Secrets::from([(CONFIG.to_owned(), serde_json::to_string(config).map_err(|e| e.to_string())?)]))
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod refresh_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::InMemorySecretStore;

    #[test]
    fn rotated_subscription_tokens_are_saved_and_loaded_back() {
        llm_providers::testing::install();
        let secrets = Arc::new(InMemorySecretStore::default());
        let slot = SecretId::new();
        let original = r#"{"auth":{"access_token":"access-secret","refresh_token":"refresh-secret","expires_at":"2099-01-01T00:00:00Z","account_id":"acct"}}"#;
        secrets.set(slot, &Secrets::from([(CONFIG.into(), original.into())])).expect("save");
        let source = AccountStore { secrets, slot, provider: ModelProvider::new("browser-sub") };
        source.save(&original.replace("refresh-secret", "refresh-rotated")).expect("rotate");
        assert!(source.load().expect("load").contains("refresh-rotated"));
        assert!(source.save("not a credential").is_err(), "a broken credential is never kept");
    }
}
