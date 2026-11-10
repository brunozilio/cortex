//! One provider account's credential: kept in the [`SecretStore`] as the
//! provider's [`Config`] JSON, rotated subscription tokens saved back.
//! Subscriptions sign in through the browser with PKCE on a loopback port.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use superfik_core::provider::ProviderKeyStore;
use superfik_core::{ConnectionId, ModelProvider, Principal};
use superfik_native::model::http::UreqHttp;
use superfik_native::model::providers::oauth;
use superfik_providers::Config;

use crate::plugins::{SecretStore, Secrets};

pub(crate) const CONFIG: &str = "config";

/// Signs in to a subscription and returns its credential [`Config`]. The
/// URL to open goes to `open`; the call blocks until the browser returns.
pub trait SignIn: Send + Sync {
    fn sign_in(&self, provider: ModelProvider, open: &mut dyn FnMut(String)) -> Result<Config, String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BrowserSignIn;

impl SignIn for BrowserSignIn {
    fn sign_in(&self, provider: ModelProvider, open: &mut dyn FnMut(String)) -> Result<Config, String> {
        let (port, path) = superfik_providers::oauth::callback(provider);
        let listener = superfik_oauth::Loopback::bind_on(port, path, true)?;
        let redirect = listener.redirect_uri();
        let verifier = random(64)?;
        let state = random(superfik_providers::oauth::state_bytes(provider))?;
        open(oauth::authorize(provider, &superfik_oauth::pkce::challenge(&verifier), &state, &redirect));
        let code = listener.wait(&state)?;
        let tokens =
            oauth::exchange(&UreqHttp, provider, &code, &state, &verifier, &redirect).map_err(|e| e.to_string())?;
        let config = Config { auth: Some(tokens), ..Config::default() };
        config.validate(provider).map_err(|e| e.to_string())?;
        Ok(config)
    }
}

fn random(length: usize) -> Result<String, String> {
    let mut bytes = vec![0; length];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) struct AccountStore {
    pub secrets: Arc<dyn SecretStore>,
    pub owner: Principal,
    pub slot: ConnectionId,
    pub provider: ModelProvider,
}

impl ProviderKeyStore for AccountStore {
    fn load(&self) -> Result<String, String> {
        let values = self.secrets.get(&self.owner, self.slot).ok_or("provider account disconnected")?;
        if let Some(config) = values.get(CONFIG) {
            return Ok(config.clone());
        }
        let key = values.get("api_key").ok_or("provider account has no credential")?;
        let config = Config::parse(self.provider, key).map_err(|e| e.to_string())?;
        serde_json::to_string(&config).map_err(|e| e.to_string())
    }

    fn save(&self, value: &str) -> Result<(), String> {
        Config::parse(self.provider, value).map_err(|e| e.to_string())?;
        let mut secrets = self.secrets.get(&self.owner, self.slot).ok_or("provider account disconnected")?;
        secrets.insert(CONFIG.into(), value.to_owned());
        self.secrets.set(&self.owner, self.slot, &secrets)
    }
}

pub(crate) fn secrets(config: &Config) -> Result<Secrets, String> {
    Ok(Secrets::from([(CONFIG.to_owned(), serde_json::to_string(config).map_err(|e| e.to_string())?)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::InMemorySecretStore;

    #[test]
    fn rotated_subscription_tokens_are_saved_and_loaded_back() {
        let secrets = Arc::new(InMemorySecretStore::default());
        let slot = ConnectionId::new();
        let original = r#"{"auth":{"access_token":"access-secret","refresh_token":"refresh-secret","expires_at":"2099-01-01T00:00:00Z","account_id":"acct"}}"#;
        secrets.set(&Principal::LocalOwner, slot, &Secrets::from([(CONFIG.into(), original.into())])).expect("save");
        let source =
            AccountStore { secrets, owner: Principal::LocalOwner, slot, provider: ModelProvider::ChatGptSubscription };
        source.save(&original.replace("refresh-secret", "refresh-rotated")).expect("rotate");
        assert!(source.load().expect("load").contains("refresh-rotated"));
        assert!(source.save("not a credential").is_err(), "a broken credential is never kept");
    }

    #[test]
    fn a_key_kept_before_configs_still_loads() {
        let secrets = Arc::new(InMemorySecretStore::default());
        let slot = ConnectionId::new();
        let kept = Secrets::from([("api_key".into(), "sk-ant-0123456789abcd".into())]);
        secrets.set(&Principal::LocalOwner, slot, &kept).expect("save");
        let source = AccountStore { secrets, owner: Principal::LocalOwner, slot, provider: ModelProvider::Anthropic };
        let config = Config::parse(ModelProvider::Anthropic, &source.load().expect("load")).expect("config");
        assert_eq!(config.api_key, "sk-ant-0123456789abcd");
    }
}
