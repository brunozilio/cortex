use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::account_pool::Balance;
use crate::model::ModelProvider;
use crate::provider::{ProviderAccount, ProviderInfo, ProviderSummary};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderBook {
    #[serde(default, deserialize_with = "one_or_many")]
    accounts: BTreeMap<ModelProvider, Vec<ProviderAccount>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    active: BTreeMap<ModelProvider, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    balance: BTreeMap<ModelProvider, Balance>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Many(Vec<ProviderAccount>),
    One(ProviderAccount),
}

fn one_or_many<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<ModelProvider, Vec<ProviderAccount>>, D::Error> {
    let stored = BTreeMap::<ModelProvider, Stored>::deserialize(deserializer)?;
    Ok(stored
        .into_iter()
        .map(|(provider, stored)| match stored {
            Stored::Many(all) => (provider, all),
            Stored::One(one) => (provider, vec![one]),
        })
        .collect())
}

impl ProviderBook {
    pub fn accounts(&self, provider: ModelProvider) -> &[ProviderAccount] {
        self.accounts.get(&provider).map_or(&[], Vec::as_slice)
    }

    pub fn get(&self, provider: ModelProvider) -> Option<&ProviderAccount> {
        let all = self.accounts(provider);
        let chosen = self.active.get(&provider);
        chosen.and_then(|id| all.iter().find(|account| &account.id == id)).or_else(|| all.first())
    }

    pub fn active_index(&self, provider: ModelProvider) -> usize {
        let id = self.get(provider).map(|account| account.id.as_str());
        self.accounts(provider).iter().position(|account| Some(account.id.as_str()) == id).unwrap_or(0)
    }

    pub fn balance(&self, provider: ModelProvider) -> Balance {
        self.balance.get(&provider).copied().unwrap_or_default()
    }

    pub fn set_balance(&mut self, provider: ModelProvider, balance: Balance) {
        match balance {
            Balance::Off => self.balance.remove(&provider),
            chosen => self.balance.insert(provider, chosen),
        };
    }

    pub fn slot_for(&self, provider: ModelProvider, label: &str, fresh: impl FnOnce() -> String) -> String {
        let all = self.accounts(provider);
        match all.iter().find(|account| account.label.eq_ignore_ascii_case(label)) {
            Some(same) => same.id.clone(),
            None if all.is_empty() => String::new(),
            None => fresh(),
        }
    }

    pub fn connect(&mut self, provider: ModelProvider, account: ProviderAccount) {
        let all = self.accounts.entry(provider).or_default();
        self.active.insert(provider, account.id.clone());
        match all.iter_mut().find(|kept| kept.id == account.id) {
            Some(kept) => *kept = account,
            None => all.push(account),
        }
    }

    pub fn use_account(&mut self, provider: ModelProvider, wanted: &str) -> Result<&ProviderAccount, String> {
        let at = self.position(provider, wanted)?;
        let id = self.accounts(provider)[at].id.clone();
        self.active.insert(provider, id);
        Ok(&self.accounts(provider)[at])
    }

    pub fn remove(&mut self, provider: ModelProvider, wanted: &str) -> Result<ProviderAccount, String> {
        let at = self.position(provider, wanted)?;
        let all = self.accounts.entry(provider).or_default();
        let removed = all.remove(at);
        if all.is_empty() {
            self.accounts.remove(&provider);
        }
        if self.active.get(&provider) == Some(&removed.id) {
            self.active.remove(&provider);
        }
        Ok(removed)
    }

    pub fn disconnect(&mut self, provider: ModelProvider) -> Vec<ProviderAccount> {
        self.active.remove(&provider);
        self.accounts.remove(&provider).unwrap_or_default()
    }

    pub fn summaries(&self, known: impl IntoIterator<Item = ProviderInfo>) -> Vec<ProviderSummary> {
        known
            .into_iter()
            .map(|info| ProviderSummary {
                account: self.get(info.provider).cloned(),
                accounts: self.accounts(info.provider).to_vec(),
                balance: self.balance(info.provider),
                provider: info.provider,
                name: info.name,
                access: info.access,
            })
            .collect()
    }

    fn position(&self, provider: ModelProvider, wanted: &str) -> Result<usize, String> {
        let all = self.accounts(provider);
        let wanted = wanted.trim();
        let numbered = wanted.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|at| *at < all.len());
        numbered
            .or_else(|| all.iter().position(|account| account.label.eq_ignore_ascii_case(wanted)))
            .ok_or_else(|| format!("no {provider} account is called {wanted}"))
    }
}

#[cfg(test)]
mod tests;
