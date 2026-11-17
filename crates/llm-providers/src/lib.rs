extern crate self as llm_providers;

pub mod added_tools;
pub mod catalog;
mod config;
pub mod declared;
pub mod model;
pub mod oauth;
pub mod providers;
pub mod request;
pub mod sign_in;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
pub mod transcript;
mod vendor;
pub mod vendors;
pub mod web_search;

pub use vendor::{Vendor, effort, owner, rated, rates, search_price, vendor};

use domain::{Access, ModelOption};
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
    #[error("the sign-in callback port {0} is already in use by another app")]
    PortInUse(u16),
}

pub trait Provider: Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn vendor(&self) -> &'static str;
    fn access(&self) -> Access;
    fn rank(&self) -> u8;
    fn slot(&self) -> u128;
    fn window(&self) -> u64;
    fn wire(&self, model: Option<&ModelOption>) -> &'static str;
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

    fn serves(&self, model: &str) -> bool {
        let model = model.rsplit(':').next().unwrap_or(model);
        vendor(self.vendor()).map_or_else(|| owner(model).is_some(), |vendor| vendor.owns(model))
    }

    fn small_model(&self) -> Option<&'static str> {
        vendor(self.vendor()).and_then(Vendor::small)
    }

    fn web_search(&self, _model: &str) -> Option<Value> {
        None
    }

    fn search(&self, _config: &Config, _query: &str) -> Option<Call> {
        None
    }

    fn hits(&self, _value: &Value) -> Result<Vec<web_search::Hit>, ProviderError> {
        Err(ProviderError::Invalid(format!("{} has no web search of its own", self.name())))
    }

    fn tools_in_conversation(&self, _model: &str) -> bool {
        false
    }

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
        _now: jiff::Timestamp,
    ) -> Result<Tokens, ProviderError> {
        Err(ProviderError::Unauthorized(format!("{} returned HTTP {status} and keeps no tokens", self.name())))
    }

    fn sign_in(&self, _host: &dyn Host) -> Result<Config, ProviderError> {
        Err(ProviderError::Invalid(format!("{} connects with an API key", self.name())))
    }
}

pub fn id(provider: &dyn Provider) -> domain::ModelProvider {
    domain::ModelProvider::new(provider.id())
}
