use crate::api::Reply;
use domain::account_pool::{self, Balance};
use domain::provider::{ModelProvider, ProviderBook, ProviderKey};
use llm_providers::Config;
use platform::sync::Locked;

use super::{ModelProviders, slot};

const SUBJECT_SHOWN: usize = 8;

pub(super) fn identity(config: &Config, name: &str) -> Option<String> {
    let tokens = config.auth.as_ref()?;
    let subject = || {
        let subject = tokens.subject.as_deref()?;
        Some(format!("{name} · {}", subject.chars().take(SUBJECT_SHOWN).collect::<String>()))
    };
    tokens.email.clone().or_else(|| (!tokens.account_id.is_empty()).then(|| tokens.account_id.clone())).or_else(subject)
}

pub(super) fn unnamed(config: &Config, label: String, kept: usize) -> String {
    match (&config.auth, kept) {
        (Some(_), 1..) => format!("{label} #{}", kept + 1),
        _ => label,
    }
}

impl ModelProviders {
    pub fn use_account(&self, provider: ModelProvider, wanted: &str) -> Reply {
        let result = self.change(&mut |book| book.use_account(provider, wanted).map(|_| ()));
        self.answer(provider, result)
    }

    pub fn remove_account(&self, provider: ModelProvider, wanted: &str) -> Reply {
        let result = self.change(&mut |book| {
            let removed = book.remove(provider, wanted)?;
            self.secrets.delete(slot(provider, &removed.id));
            Ok(())
        });
        self.answer(provider, result)
    }

    pub fn set_balance(&self, provider: ModelProvider, balance: Balance) -> Reply {
        let result = self.change(&mut |book| {
            book.set_balance(provider, balance);
            Ok(())
        });
        self.answer(provider, result)
    }

    pub fn keys(&self, provider: ModelProvider) -> Result<Vec<(String, ProviderKey)>, String> {
        let first = (agent_engine::providers::name(provider), self.key(provider)?);
        let book = self.store.load();
        let accounts = book.accounts(provider);
        let run = {
            let mut runs = self.runs.locked();
            let counter = runs.entry(provider).or_default();
            *counter += 1;
            *counter - 1
        };
        let order = account_pool::order(book.balance(provider), accounts.len(), book.active_index(provider), run);
        let keys: Vec<(String, ProviderKey)> = order
            .into_iter()
            .filter_map(|at| {
                let key = self.account_key(provider, &accounts[at].id).ok()?;
                Some((accounts[at].label.clone(), key))
            })
            .collect();
        Ok(if keys.is_empty() { vec![first] } else { keys })
    }

    pub fn disconnect(&self, provider: ModelProvider) -> Reply {
        let result = self.change(&mut |book| {
            for account in book.disconnect(provider) {
                self.secrets.delete(slot(provider, &account.id));
            }
            Ok(())
        });
        self.answer(provider, result)
    }

    fn change(&self, change: &mut dyn FnMut(&mut ProviderBook) -> Result<(), String>) -> Result<(), String> {
        let _guard = self.lock.locked();
        let mut outcome = Ok(());
        self.store
            .update(&mut |book| {
                let mut changed = book.clone();
                outcome = change(&mut changed);
                if outcome.is_ok() {
                    *book = changed;
                }
            })
            .map_err(|error| error.to_string())?;
        outcome
    }
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
