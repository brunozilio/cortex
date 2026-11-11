use std::collections::BTreeMap;
use std::sync::{PoisonError, RwLock};

use domain::{Access, ModelOption, ModelProvider};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::catalog::{self, Call, Page, STANDARD};
use crate::{Config, Provider, ProviderError};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Declared {
    pub id: String,
    pub name: String,
    pub wire: String,
    pub base: String,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub auth_prefix: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub window: Option<u64>,
}

pub type Shaper = Box<dyn Fn(&mut Value) -> Result<(), String> + Send + Sync>;

struct DeclaredProvider {
    shaper: Option<Shaper>,
    id: &'static str,
    name: &'static str,
    wire: &'static str,
    base: String,
    auth_header: String,
    auth_prefix: String,
    headers: BTreeMap<String, String>,
    models: Vec<String>,
    window: u64,
}

static REGISTERED: RwLock<Vec<&'static dyn Provider>> = RwLock::new(Vec::new());

pub fn registered() -> Vec<&'static dyn Provider> {
    REGISTERED.read().unwrap_or_else(PoisonError::into_inner).clone()
}

pub fn find(provider: ModelProvider) -> Option<&'static dyn Provider> {
    registered().into_iter().find(|known| known.id() == provider.id())
}

impl std::fmt::Debug for DeclaredProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeclaredProvider").field("id", &self.id).field("shaped", &self.shaper.is_some()).finish()
    }
}

pub fn register(declared: Declared) -> Result<ModelProvider, ProviderError> {
    register_shaped(declared, None)
}

pub fn register_shaped(declared: Declared, shaper: Option<Shaper>) -> Result<ModelProvider, ProviderError> {
    let provider = DeclaredProvider { shaper, ..check(declared)? };
    let mut registered = REGISTERED.write().unwrap_or_else(PoisonError::into_inner);
    registered.retain(|known| known.id() != provider.id);
    let id = ModelProvider::named(provider.id);
    registered.push(Box::leak(Box::new(provider)));
    Ok(id)
}

fn check(declared: Declared) -> Result<DeclaredProvider, ProviderError> {
    let invalid = |why: String| Err(ProviderError::Invalid(format!("provider {:?}: {why}", declared.id)));
    let id_shaped = !declared.id.is_empty()
        && declared.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !id_shaped {
        return invalid("the id takes lowercase letters, digits and dashes".into());
    }
    let wire = match declared.wire.as_str() {
        "chat" => "chat",
        "responses" => "responses",
        "messages" => "messages",
        other => return invalid(format!("wire {other:?} is not chat, responses or messages")),
    };
    let base = declared.base.trim_end_matches('/').to_owned();
    let local = ["http://localhost", "http://127.0.0.1"].iter().any(|start| base.starts_with(start));
    if !base.starts_with("https://") && !local {
        return invalid("base must be https, or http on localhost".into());
    }
    Ok(DeclaredProvider {
        shaper: None,
        id: Box::leak(declared.id.into_boxed_str()),
        name: Box::leak(declared.name.into_boxed_str()),
        wire,
        base,
        auth_header: declared.auth_header.unwrap_or_else(|| "authorization".into()),
        auth_prefix: declared.auth_prefix.unwrap_or_else(|| "Bearer ".into()),
        headers: declared.headers,
        models: declared.models,
        window: declared.window.unwrap_or(128_000),
    })
}

impl Provider for DeclaredProvider {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn vendor(&self) -> &'static str {
        "declared"
    }
    fn access(&self) -> Access {
        Access::Key
    }
    fn rank(&self) -> u8 {
        200
    }
    fn slot(&self) -> u128 {
        let digest = Sha256::digest(format!("declared provider {}", self.id));
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&digest[..16]);
        u128::from_be_bytes(bytes)
    }
    fn window(&self) -> u64 {
        self.window
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        self.wire
    }
    fn base(&self, _config: &Config) -> String {
        self.base.clone()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        let mut headers = vec![
            ("accept".to_owned(), "text/event-stream".to_owned()),
            (self.auth_header.clone(), format!("{}{}", self.auth_prefix, config.api_key)),
        ];
        if self.wire == "messages" {
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
        }
        headers.extend(self.headers.iter().map(|(name, value)| (name.clone(), value.clone())));
        headers
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        match self.wire {
            "responses" => "/responses",
            "messages" => "/messages",
            _ => "/chat/completions",
        }
    }
    fn shape(&self, _model: Option<&ModelOption>, body: &mut Value, _effort: &str) -> Result<(), ProviderError> {
        match &self.shaper {
            Some(shaper) => shaper(body).map_err(|why| ProviderError::Invalid(format!("{}: shape: {why}", self.id))),
            None => Ok(()),
        }
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        Call::get(self, config, "/models")
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let models = if self.models.is_empty() {
            catalog::items(value, "data")?.iter().filter_map(|item| catalog::model(self, item, &STANDARD)).collect()
        } else {
            self.models.iter().map(|id| catalog::option(self, id, format!("{id} ({})", self.name))).collect()
        };
        Ok(Page { models, next: None })
    }
}

#[cfg(test)]
#[path = "declared_tests.rs"]
mod tests;
