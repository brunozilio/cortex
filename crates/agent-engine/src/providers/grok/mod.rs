pub mod device;
mod sign_in;

use chrono::{DateTime, Utc};
use domain::{Access, ModelOption};
use serde_json::Value;

use llm_providers::catalog::{self, Call, Page};
use llm_providers::oauth::{self, TokenRequest, Tokens};
use llm_providers::{Config, Host, Provider as Shape, ProviderError};

pub const API: &str = "https://api.x.ai/v1";
const SKIPPED: [&str; 5] = ["imagine", "image", "embedding", "tts", "video"];

#[derive(Debug, Clone, Copy, Default)]
pub struct Provider;

impl Shape for Provider {
    fn id(&self) -> &'static str {
        "grok-subscription"
    }
    fn name(&self) -> &'static str {
        "Grok Subscription"
    }
    fn vendor(&self) -> &'static str {
        "xai"
    }
    fn access(&self) -> Access {
        Access::Code
    }
    fn rank(&self) -> u8 {
        70
    }
    fn slot(&self) -> u128 {
        0x7072_6f76_6964_6572_0000_0000_0000_0008
    }
    fn window(&self) -> u64 {
        500_000
    }
    fn wire(&self, _model: Option<&ModelOption>) -> &'static str {
        "responses"
    }
    fn check(&self, config: &Config) -> Result<(), ProviderError> {
        config.refreshable().map(|_| ())
    }
    fn base(&self, _config: &Config) -> String {
        API.to_owned()
    }
    fn headers(&self, config: &Config) -> Vec<(String, String)> {
        let mut headers = vec![("accept".to_owned(), "text/event-stream".to_owned())];
        if config.auth.is_some() {
            headers.push(("authorization".into(), format!("Bearer {}", config.access_token())));
        }
        headers
    }
    fn path(&self, _model: Option<&ModelOption>) -> &'static str {
        "/responses"
    }
    fn retries_after_refresh(&self, status: u16) -> bool {
        status == 401
    }
    fn shape(&self, model: Option<&ModelOption>, body: &mut Value, effort: &str) -> Result<(), ProviderError> {
        llm_providers::effort(self.vendor(), body, model, effort, false)
    }
    fn catalog(&self, config: &Config, _after: &str) -> Call {
        Call::get(self, config, "/models")
    }
    fn page(&self, value: &Value) -> Result<Page, ProviderError> {
        let models = catalog::items(value, "data")?.iter().filter_map(|item| self.model(item));
        Ok(Page { models: models.collect(), next: None })
    }
    fn finish(&self, mut models: Vec<ModelOption>) -> Vec<ModelOption> {
        models.sort_by(|a, b| b.id.cmp(&a.id));
        models
    }
    fn refresh(&self, previous: &Tokens) -> Option<TokenRequest> {
        Some(TokenRequest::form(
            device::TOKEN,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", device::CLIENT),
                ("refresh_token", previous.refresh_token.as_str()),
            ],
        ))
    }
    fn tokens(
        &self,
        status: u16,
        body: &str,
        previous: Option<&Tokens>,
        now: DateTime<Utc>,
    ) -> Result<Tokens, ProviderError> {
        oauth::standard(status, body, previous, now, |_| Ok(String::new()))
    }
    fn sign_in(&self, host: &dyn Host) -> Result<Config, ProviderError> {
        sign_in::sign_in(host)
    }
}

impl Provider {
    fn model(&self, item: &Value) -> Option<ModelOption> {
        let id = catalog::string(item, "id");
        let lower = id.to_ascii_lowercase();
        if !lower.starts_with("grok") || SKIPPED.iter().any(|part| lower.contains(part)) {
            return None;
        }
        let mut model = catalog::option(self, id, format!("{id} (Grok)"));
        model.window = window(item);
        let reasons = item["supports_reasoning_effort"]
            .as_bool()
            .unwrap_or_else(|| llm_providers::vendor(self.vendor()).is_some_and(|vendor| vendor.reasons(id)));
        model.capabilities = Some(domain::model::ModelCapabilities { reasoning: reasons, ..Default::default() });
        Some(model)
    }
}

fn window(item: &Value) -> Option<u64> {
    let largest = item["context_windows"].as_array().and_then(|all| all.iter().filter_map(Value::as_u64).max());
    largest
        .or_else(|| item["context_window"].as_u64())
        .or_else(|| item["context_length"].as_u64())
        .filter(|n| *n > 0)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
mod client_tests;
