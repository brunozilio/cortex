use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use crate::model::{Access, ModelProvider};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub label: String,
    pub since_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub provider: ModelProvider,
    pub name: String,
    pub access: Access,
    pub account: Option<ProviderAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    pub provider: ModelProvider,
    pub name: String,
    pub access: Access,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderBook {
    #[serde(default)]
    accounts: BTreeMap<ModelProvider, ProviderAccount>,
}

impl ProviderBook {
    pub fn get(&self, provider: ModelProvider) -> Option<&ProviderAccount> {
        self.accounts.get(&provider)
    }

    pub fn connect(&mut self, provider: ModelProvider, account: ProviderAccount) -> Option<ProviderAccount> {
        self.accounts.insert(provider, account)
    }

    pub fn disconnect(&mut self, provider: ModelProvider) -> Option<ProviderAccount> {
        self.accounts.remove(&provider)
    }

    pub fn summaries(&self, known: impl IntoIterator<Item = ProviderInfo>) -> Vec<ProviderSummary> {
        known
            .into_iter()
            .map(|info| ProviderSummary {
                account: self.get(info.provider).cloned(),
                provider: info.provider,
                name: info.name,
                access: info.access,
            })
            .collect()
    }
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

pub trait ProviderKeyStore: Send + Sync {
    fn load(&self) -> Result<String, String>;
    fn save(&self, value: &str) -> Result<(), String>;
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

    fn account(label: &str) -> ProviderAccount {
        ProviderAccount { label: label.into(), since_unix: 10 }
    }

    #[test]
    fn one_account_per_provider_and_the_new_one_replaces_the_old() {
        let mut book = ProviderBook::default();
        assert_eq!(book.connect(ModelProvider::new("openai"), account("…ab12")), None);
        let old = book.connect(ModelProvider::new("openai"), account("…cd34"));
        assert_eq!(old.map(|a| a.label), Some("…ab12".into()));
        assert_eq!(book.get(ModelProvider::new("openai")).map(|a| a.label.as_str()), Some("…cd34"));
        assert_eq!(book.disconnect(ModelProvider::new("openai")).map(|a| a.label), Some("…cd34".into()));
        assert_eq!(book.get(ModelProvider::new("openai")), None);
    }

    #[test]
    fn summaries_list_every_provider_with_an_account_in_order() {
        let mut book = ProviderBook::default();
        let (one, two) = (ModelProvider::new("one"), ModelProvider::new("two"));
        book.connect(two, account("…ff00"));
        let info = |provider, name: &str| ProviderInfo { provider, name: name.into(), access: Access::Key };
        let summaries = book.summaries([info(one, "One"), info(two, "Two")]);
        assert_eq!(summaries.iter().map(|s| s.provider).collect::<Vec<_>>(), [one, two]);
        assert_eq!(summaries[0].account, None);
        assert_eq!(
            (summaries[1].name.as_str(), summaries[1].account.as_ref().map(|a| a.label.as_str())),
            ("Two", Some("…ff00"))
        );
    }

    #[test]
    fn a_book_round_trips() {
        let mut book = ProviderBook::default();
        book.connect(ModelProvider::new("anthropic"), account("x"));
        let back: ProviderBook = serde_json::from_str(&serde_json::to_string(&book).expect("json")).expect("parse");
        assert_eq!(back, book);
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
