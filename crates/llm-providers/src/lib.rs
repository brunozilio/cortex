pub mod catalog;
mod config;
pub mod declared;
pub mod oauth;
pub mod request;
pub mod sign_in;
#[cfg(test)]
mod testing;

pub mod providers {
    include!(concat!(env!("OUT_DIR"), "/registry.rs"));
}

use domain::provider::ProviderInfo;
use domain::{Access, ModelOption, ModelProvider};
use serde_json::Value;

pub use catalog::{Call, Page};
pub use config::Config;
pub use oauth::{TokenRequest, Tokens};
pub use sign_in::Host;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("not signed in: {0}")]
    Unauthorized(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    Messages,
    Responses,
    Chat,
    EventStream,
}

pub trait Provider: Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn vendor(&self) -> &'static str;
    fn access(&self) -> Access;
    fn rank(&self) -> u8;
    fn slot(&self) -> u128;
    fn window(&self) -> u64;
    fn wire(&self, model: Option<&ModelOption>) -> Wire;
    fn base(&self, config: &Config) -> String;
    fn headers(&self, config: &Config) -> Vec<(String, String)>;
    fn path(&self, model: Option<&ModelOption>) -> &'static str;
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError>;
    fn catalog(&self, config: &Config, after: &str) -> Call;
    fn page(&self, value: &Value) -> Result<Page, ProviderError>;

    fn bare(&self, raw: &str) -> Config {
        Config { api_key: raw.trim().to_owned(), ..Config::default() }
    }

    fn check(&self, _config: &Config) -> Result<(), ProviderError> {
        Ok(())
    }

    fn bind(&self, _config: &Config, _body: &mut Value) {}

    fn turn_headers(&self, _model: Option<&ModelOption>, _body: &Value) -> Vec<(String, String)> {
        Vec::new()
    }

    fn retries_after_refresh(&self, _status: u16) -> bool {
        false
    }

    fn finish(&self, models: Vec<ModelOption>) -> Vec<ModelOption> {
        models
    }

    fn refresh(&self, _previous: &Tokens) -> Option<TokenRequest> {
        None
    }

    fn tokens(
        &self,
        status: u16,
        _body: &str,
        _previous: Option<&Tokens>,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Tokens, ProviderError> {
        Err(ProviderError::Unauthorized(format!("{} returned HTTP {status} and keeps no tokens", self.name())))
    }

    fn sign_in(&self, _host: &dyn Host) -> Result<Config, ProviderError> {
        Err(ProviderError::Invalid(format!("{} connects with an API key", self.name())))
    }
}

pub fn all() -> Vec<&'static dyn Provider> {
    let mut all = providers::ALL.to_vec();
    all.extend(declared::registered());
    all.sort_by_key(|provider| provider.rank());
    all
}

pub fn find(provider: ModelProvider) -> Option<&'static dyn Provider> {
    providers::ALL
        .iter()
        .copied()
        .find(|known| known.id() == provider.id())
        .or_else(|| declared::registered().into_iter().find(|known| known.id() == provider.id()))
}

pub fn get(provider: ModelProvider) -> Result<&'static dyn Provider, ProviderError> {
    find(provider).ok_or_else(|| ProviderError::Invalid(format!("no provider is called {provider}")))
}

pub fn id(provider: &dyn Provider) -> ModelProvider {
    ModelProvider::new(provider.id())
}

pub fn infos() -> Vec<ProviderInfo> {
    all()
        .into_iter()
        .map(|provider| ProviderInfo {
            provider: id(provider),
            name: provider.name().to_owned(),
            access: provider.access(),
        })
        .collect()
}

pub fn name(provider: ModelProvider) -> String {
    find(provider).map_or_else(|| provider.id().to_owned(), |known| known.name().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_provider_has_its_own_id_rank_and_secret_slot() {
        let mut all = providers::ALL.to_vec();
        all.sort_by_key(|provider| provider.rank());
        let mut ids: Vec<_> = all.iter().map(|p| p.id()).collect();
        let mut slots: Vec<_> = all.iter().map(|p| p.slot()).collect();
        ids.dedup();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!((ids.len(), slots.len()), (all.len(), all.len()));
        assert!(all.windows(2).all(|pair| pair[0].rank() < pair[1].rank()));
    }

    #[test]
    fn an_unknown_provider_is_named_by_its_id_and_cannot_be_used() {
        let unknown = ModelProvider::new("nobody");
        assert_eq!(name(unknown), "nobody");
        assert!(get(unknown).is_err());
    }
}
