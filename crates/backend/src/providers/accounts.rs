use domain::Principal;
use domain::account_pool::{self, Balance};
use domain::provider::{ModelProvider, ProviderBook, ProviderKey};
use llm_providers::Config;
use platform::sync::Locked;
use protocol::ServerEvent;

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
    pub fn use_account(&self, owner: &Principal, provider: ModelProvider, wanted: &str) -> ServerEvent {
        let result = self.change(owner, &mut |book| book.use_account(provider, wanted).map(|_| ()));
        self.answer(owner, provider, result)
    }

    pub fn remove_account(&self, owner: &Principal, provider: ModelProvider, wanted: &str) -> ServerEvent {
        let result = self.change(owner, &mut |book| {
            let removed = book.remove(provider, wanted)?;
            self.secrets.delete(owner, slot(provider, &removed.id));
            Ok(())
        });
        self.answer(owner, provider, result)
    }

    pub fn set_balance(&self, owner: &Principal, provider: ModelProvider, balance: Balance) -> ServerEvent {
        let result = self.change(owner, &mut |book| {
            book.set_balance(provider, balance);
            Ok(())
        });
        self.answer(owner, provider, result)
    }

    pub fn keys(&self, owner: &Principal, provider: ModelProvider) -> Result<Vec<ProviderKey>, String> {
        let first = self.key(owner, provider)?;
        let Some(holder) = self.holder(owner, provider) else { return Ok(vec![first]) };
        let book = self.store.load(&holder);
        let accounts = book.accounts(provider);
        let run = {
            let mut runs = self.runs.locked();
            let counter = runs.entry(format!("{}/{provider}", crate::dirs::owner_key(&holder))).or_default();
            *counter += 1;
            *counter - 1
        };
        let order = account_pool::order(book.balance(provider), accounts.len(), book.active_index(provider), run);
        let keys: Vec<ProviderKey> = order
            .into_iter()
            .filter_map(|at| self.account_key(holder.clone(), provider, &accounts[at].id).ok())
            .collect();
        Ok(if keys.is_empty() { vec![first] } else { keys })
    }

    pub fn disconnect(&self, owner: &Principal, provider: ModelProvider) -> ServerEvent {
        let result = self.change(owner, &mut |book| {
            for account in book.disconnect(provider) {
                self.secrets.delete(owner, slot(provider, &account.id));
            }
            Ok(())
        });
        self.answer(owner, provider, result)
    }

    fn change(
        &self,
        owner: &Principal,
        change: &mut dyn FnMut(&mut ProviderBook) -> Result<(), String>,
    ) -> Result<(), String> {
        let _guard = self.lock.locked();
        let mut outcome = Ok(());
        self.store
            .update(owner, &mut |book| {
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
