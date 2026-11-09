//! Logins for the browser card, as the Bitwarden extension offers them:
//! the items whose saved addresses belong to the page, then the values of
//! the one the user picks, checked against the page again. The user picks
//! here, so nothing is asked; each owner's unlocked sessions are kept
//! between fills so `bw unlock` runs once per account, and synced every
//! [`SYNC_EVERY`] so logins saved elsewhere show up.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use superfik_core::Principal;
use superfik_core::autofill::{Candidate, LoginFill, NewLogin, matches_any};

use super::provider::{Item, VaultAccount, VaultProviders};

pub const SYNC_EVERY: Duration = Duration::from_secs(120);

struct Kept {
    session: Option<String>,
    synced: Instant,
}

type Sessions = HashMap<(Principal, String), Kept>;

#[derive(Clone)]
pub struct Logins {
    providers: VaultProviders,
    sessions: Arc<Mutex<Sessions>>,
    sync_every: Duration,
}

impl Default for Logins {
    fn default() -> Self {
        Self::new(VaultProviders::default())
    }
}

impl std::fmt::Debug for Logins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logins").finish_non_exhaustive()
    }
}

impl Logins {
    pub fn new(providers: VaultProviders) -> Self {
        Self { providers, sessions: Arc::default(), sync_every: SYNC_EVERY }
    }

    #[cfg(test)]
    pub fn syncing_every(self, every: Duration) -> Self {
        Self { sync_every: every, ..self }
    }

    pub fn candidates(
        &self,
        owner: &Principal,
        accounts: &[VaultAccount],
        url: &str,
    ) -> Result<Vec<Candidate>, String> {
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut problems = Vec::new();
        for account in accounts {
            match self.items(owner, account, Reach::Vault(url)) {
                Ok(items) => {
                    for item in items.iter().filter(|item| belongs(item, url)) {
                        let username = item.values.get("username").cloned();
                        if !candidates.iter().any(|c| c.item == item.name && c.username == username) {
                            let account = account.title.clone();
                            candidates.push(Candidate { item: item.name.clone(), account, username });
                        }
                    }
                }
                Err(error) => problems.push(error),
            }
        }
        if candidates.is_empty() && !problems.is_empty() {
            return Err(problems.join("\n"));
        }
        Ok(candidates)
    }

    pub fn fill(
        &self,
        owner: &Principal,
        accounts: &[VaultAccount],
        url: &str,
        account: &str,
        item: &str,
        scoped: bool,
    ) -> Result<LoginFill, String> {
        let account =
            accounts.iter().find(|a| a.title == account).ok_or_else(|| format!("{account} is not connected"))?;
        let reach = if scoped { Reach::Scope } else { Reach::Vault(url) };
        let items = self.items(owner, account, reach)?;
        let found = items
            .iter()
            .find(|i| i.name == item && belongs(i, url))
            .ok_or_else(|| format!("{item} is not saved for this page"))?;
        let value = |field: &str| found.values.get(field).cloned();
        Ok(LoginFill { username: value("username"), password: value("password") })
    }

    pub fn save(
        &self,
        owner: &Principal,
        accounts: &[VaultAccount],
        account: &str,
        login: &NewLogin,
    ) -> Result<(), String> {
        let account =
            accounts.iter().find(|a| a.title == account).ok_or_else(|| format!("{account} is not connected"))?;
        let provider = self
            .providers
            .get(account.kind)
            .ok_or_else(|| format!("{}: this vault is not supported", account.title))?;
        let key = (owner.clone(), account.title.clone());
        let kept = self.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&key);
        let synced = kept.as_ref().map_or_else(Instant::now, |k| k.synced);
        let mut session = kept.and_then(|k| k.session);
        let saved = provider.save(account, &mut session, login).map_err(|error| format!("{}: {error}", account.title));
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).insert(key, Kept { session, synced });
        saved
    }

    fn items(&self, owner: &Principal, account: &VaultAccount, reach: Reach<'_>) -> Result<Vec<Item>, String> {
        self.opened(owner, account, |session| match reach {
            Reach::Scope => provider_items(&self.providers, account, session),
            Reach::Vault(url) => provider_logins(&self.providers, account, session, url),
        })
    }

    fn opened<T>(
        &self,
        owner: &Principal,
        account: &VaultAccount,
        list: impl Fn(&mut Option<String>) -> Result<T, String>,
    ) -> Result<T, String> {
        let provider = self
            .providers
            .get(account.kind)
            .ok_or_else(|| format!("{}: this vault is not supported", account.title))?;
        let key = (owner.clone(), account.title.clone());
        let kept = self.sessions.lock().unwrap_or_else(PoisonError::into_inner).remove(&key);
        let mut synced = kept.as_ref().map_or_else(Instant::now, |k| k.synced);
        let mut session = kept.and_then(|k| k.session);
        if let Some(key) = session.as_deref().filter(|_| synced.elapsed() >= self.sync_every) {
            if let Err(error) = provider.refresh(account, key) {
                tracing::debug!(%error, "vault sync failed");
            }
            synced = Instant::now();
        }
        let was_kept = session.is_some();
        let mut items = list(&mut session);
        if items.is_err() && was_kept {
            session = None;
            synced = Instant::now();
            items = list(&mut session);
        }
        let items = items.map_err(|error| format!("{}: {error}", account.title));
        if items.is_ok() {
            self.sessions.lock().unwrap_or_else(PoisonError::into_inner).insert(key, Kept { session, synced });
        }
        items
    }
}

#[derive(Clone, Copy)]
enum Reach<'a> {
    Scope,
    Vault(&'a str),
}

fn provider_items(
    providers: &VaultProviders,
    account: &VaultAccount,
    session: &mut Option<String>,
) -> Result<Vec<Item>, String> {
    providers.get(account.kind).ok_or("this vault is not supported")?.items(account, session)
}

fn provider_logins(
    providers: &VaultProviders,
    account: &VaultAccount,
    session: &mut Option<String>,
    url: &str,
) -> Result<Vec<Item>, String> {
    providers.get(account.kind).ok_or("this vault is not supported")?.logins(account, session, url)
}

fn belongs(item: &Item, url: &str) -> bool {
    (item.values.contains_key("password") || item.values.contains_key("username")) && matches_any(&item.uris, url)
}

mod passkey_create;
mod passkeys;
#[cfg(test)]
mod tests;
