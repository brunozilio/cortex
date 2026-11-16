use domain::Principal;
use domain::provider::{ModelProvider, ProviderBook};
use llm_providers::Config;
use platform::sync::Locked;
use protocol::ServerEvent;

use super::{ModelProviders, slot};

pub(super) fn identity(config: &Config) -> Option<String> {
    let tokens = config.auth.as_ref()?;
    tokens.email.clone().or_else(|| (!tokens.account_id.is_empty()).then(|| tokens.account_id.clone()))
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
