//! Getting in: the first access (the code, used once, and the password the
//! user chooses), then name and password. Each answers a sign-in token.
//! Too many failures in a row lock the name for a while.

use std::collections::HashMap;

use crate::directory::{Directory, IdentityError, Result, TOKEN_LIFETIME, username};
use crate::secret::{MIN_PASSWORD, UNKNOWN_ACCOUNT_HASH, code_hash, hash_password, password_matches, random_token};
use crate::store::{AccountStore, Token};

pub const FAILURES_BEFORE_LOCK: u32 = 5;
pub const LOCK_SECONDS: u64 = 15 * 60;
pub const MAX_TRACKED: usize = 10_000;

impl<S: AccountStore> Directory<S> {
    pub fn first_access(&self, name: &str, code: &str, password: &str, now: u64) -> Result<String> {
        if password.chars().count() < MIN_PASSWORD {
            return Err(IdentityError::WeakPassword);
        }
        self.throttled(name, now, || {
            let mut account = self.by_code(name, code, now)?;
            account.password =
                Some(hash_password(password).ok_or_else(|| IdentityError::Store("hashing failed".to_owned()))?);
            account.first_access = None;
            let user = account.user;
            self.store.put_account(account)?;
            self.token_for(user, now)
        })
    }

    pub fn sign_in(&self, name: &str, password: &str, now: u64) -> Result<String> {
        self.throttled(name, now, || {
            let account = username(name).ok().and_then(|name| self.store.account_named(self.org, &name).ok().flatten());
            let hash = account.as_ref().and_then(|account| account.password.as_deref());
            let matches = password_matches(password, hash.unwrap_or(UNKNOWN_ACCOUNT_HASH));
            match account {
                Some(account) if matches && hash.is_some() => self.token_for(account.user, now),
                _ => Err(IdentityError::WrongCredentials),
            }
        })
    }

    pub fn sign_out(&self, token: &str) -> Result<()> {
        Ok(self.store.drop_token(&code_hash(token))?)
    }

    fn token_for(&self, user: superfik_core::UserId, now: u64) -> Result<String> {
        let token = random_token().ok_or_else(|| IdentityError::Store("no randomness".to_owned()))?;
        self.store.put_token(code_hash(&token), Token { user, org: self.org, expires_unix: now + TOKEN_LIFETIME })?;
        Ok(token)
    }

    pub(crate) fn throttled<T>(&self, name: &str, now: u64, checked: impl FnOnce() -> Result<T>) -> Result<T> {
        let Ok(key) = username(name) else { return checked() };
        self.admit(&key, now)?;
        let result = checked();
        self.settle(key, now, &result);
        result
    }

    fn admit(&self, key: &str, now: u64) -> Result<()> {
        let mut attempts = self.tally();
        if !attempts.contains_key(key) && attempts.len() >= MAX_TRACKED {
            attempts.retain(|_, entry| !entry.stale(now));
            if attempts.len() >= MAX_TRACKED {
                return Err(IdentityError::TooManyAttempts);
            }
        }
        let entry = attempts.entry(key.to_owned()).or_default();
        if entry.stale(now) {
            *entry = Attempts::default();
        }
        if now < entry.locked_until || entry.failures >= FAILURES_BEFORE_LOCK {
            return Err(IdentityError::TooManyAttempts);
        }
        entry.failures += 1;
        entry.last = now;
        Ok(())
    }

    fn settle<T>(&self, key: String, now: u64, result: &Result<T>) {
        let mut attempts = self.tally();
        match result {
            Ok(_) => {
                attempts.remove(&key);
            }
            Err(IdentityError::WrongCredentials | IdentityError::WrongCode) => {
                let entry = attempts.entry(key).or_default();
                entry.last = now;
                if entry.failures >= FAILURES_BEFORE_LOCK {
                    *entry = Attempts { failures: 0, last: now, locked_until: now + LOCK_SECONDS };
                }
            }
            Err(_) => {
                if let Some(entry) = attempts.get_mut(&key) {
                    entry.failures = entry.failures.saturating_sub(1);
                }
            }
        }
    }

    fn tally(&self) -> std::sync::MutexGuard<'_, HashMap<String, Attempts>> {
        self.attempts.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[cfg(test)]
    pub(crate) fn tracked(&self) -> usize {
        self.tally().len()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Attempts {
    failures: u32,
    last: u64,
    locked_until: u64,
}

impl Attempts {
    fn stale(&self, now: u64) -> bool {
        now >= self.locked_until && now >= self.last.saturating_add(LOCK_SECONDS)
    }
}
