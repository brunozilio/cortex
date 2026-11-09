//! Explicit, bounded page conditions; no arbitrary sleep success.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserWait {
    pub text: Option<String>,
    pub url: Option<String>,
    pub reference: Option<u32>,
    pub hidden: bool,
    pub timeout_ms: u32,
}
impl Default for BrowserWait {
    fn default() -> Self {
        Self { text: None, url: None, reference: None, hidden: false, timeout_ms: 10_000 }
    }
}
impl BrowserWait {
    pub fn validate(&self) -> Result<(), String> {
        if !(100..=30_000).contains(&self.timeout_ms) {
            return Err("timeout_ms must be between 100 and 30000".into());
        }
        if self.hidden && self.reference.is_none() {
            return Err("hidden requires an element reference".into());
        }
        if self.reference == Some(0) {
            return Err("reference must be positive".into());
        }
        for value in [&self.text, &self.url].into_iter().flatten() {
            if value.is_empty() || value.len() > 4000 {
                return Err("Wait conditions must contain 1–4000 bytes".into());
            }
        }
        Ok(())
    }
}
