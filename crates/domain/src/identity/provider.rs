use serde::{Deserialize, Serialize};

pub use crate::model::{Access, ModelProvider};
pub use crate::provider_book::ProviderBook;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub label: String,
    pub since_unix: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortHolder {
    pub pid: u32,
    pub app: String,
}

pub fn app_name(executable: &str) -> String {
    let mut parts = executable.split('/');
    let mut outermost_app = parts.clone().filter_map(|part| part.strip_suffix(".app"));
    let file = parts.rfind(|part| !part.is_empty()).unwrap_or(executable);
    outermost_app.next().unwrap_or(file).to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub provider: ModelProvider,
    pub name: String,
    pub access: Access,
    pub account: Option<ProviderAccount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<ProviderAccount>,
    #[serde(default)]
    pub balance: crate::account_pool::Balance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    pub provider: ModelProvider,
    pub name: String,
    pub access: Access,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyProblem {
    Empty,
    Whitespace,
}

impl std::fmt::Display for KeyProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "the key is empty",
            Self::Whitespace => "the key has spaces in it",
        })
    }
}

pub fn clean_key(raw: &str) -> Result<String, KeyProblem> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(KeyProblem::Empty);
    }
    if key.chars().any(char::is_whitespace) {
        return Err(KeyProblem::Whitespace);
    }
    Ok(key.to_owned())
}

pub fn key_hint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 12 {
        return "…".to_owned();
    }
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("…{tail}")
}

pub type RefreshHold = Box<dyn Send>;

pub trait ProviderKeyStore: Send + Sync {
    fn load(&self) -> Result<String, String>;
    fn save(&self, value: &str) -> Result<(), String>;
    fn hold_for_refresh(&self) -> Result<Option<RefreshHold>, String> {
        Ok(None)
    }
}

#[derive(Clone)]
pub struct ProviderKey(String, Option<std::sync::Arc<dyn ProviderKeyStore>>);

impl PartialEq for ProviderKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for ProviderKey {}

impl ProviderKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into(), None)
    }

    pub fn with_store(mut self, store: std::sync::Arc<dyn ProviderKeyStore>) -> Self {
        self.1 = Some(store);
        self
    }

    pub fn current(&self) -> Result<Self, String> {
        match &self.1 {
            Some(store) => Ok(Self::new(store.load()?)),
            None => Ok(self.clone()),
        }
    }

    pub fn replace(&self, value: &str) -> Result<(), String> {
        self.1.as_ref().map_or(Ok(()), |store| store.save(value))
    }

    pub fn hold_for_refresh(&self) -> Result<Option<RefreshHold>, String> {
        self.1.as_ref().map_or(Ok(None), |store| store.hold_for_refresh())
    }

    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ProviderKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProviderKey(…)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_holder_is_named_by_the_app_that_contains_its_executable() {
        let helper =
            "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper";
        assert_eq!(app_name(helper), "Visual Studio Code");
        assert_eq!(app_name("/usr/local/bin/node"), "node");
        assert_eq!(app_name("node"), "node");
    }

    #[test]
    fn keys_are_trimmed_and_blank_or_broken_ones_refused() {
        assert_eq!(clean_key("  sk-abc\n"), Ok("sk-abc".into()));
        assert_eq!(clean_key(" \t "), Err(KeyProblem::Empty));
        assert_eq!(clean_key("sk-a bc"), Err(KeyProblem::Whitespace));
    }

    #[test]
    fn a_key_is_named_by_its_last_four_characters() {
        assert_eq!(key_hint("sk-proj-0123456789ab12"), "…ab12");
        assert_eq!(key_hint("short"), "…", "a short key gives nothing away");
    }

    #[test]
    fn keys_never_print() {
        let printed = format!("{:?}", ProviderKey::new("sk-secret"));
        assert!(!printed.contains("secret"), "{printed}");
        assert_eq!(ProviderKey::new("sk-secret").secret(), "sk-secret");
    }
}
