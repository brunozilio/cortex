//! What the directory keeps, behind a trait: in memory for tests and the
//! desktop's embedded server, a file (`FileAccounts`) for a hosted one.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use superfik_core::access::OrgRole;
use superfik_core::{OrgId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub user: UserId,
    pub org: OrgId,
    pub name: String,
    pub role: OrgRole,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub first_access: Option<FirstAccess>,
    pub active: bool,
}

impl Account {
    pub fn pending(&self) -> bool {
        self.password.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstAccess {
    pub code_hash: String,
    pub expires_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub user: UserId,
    pub org: OrgId,
    pub expires_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the account store failed: {0}")]
pub struct StoreError(pub String);

pub trait AccountStore: Send + Sync {
    fn account(&self, user: UserId) -> Result<Option<Account>, StoreError>;
    fn account_named(&self, org: OrgId, name: &str) -> Result<Option<Account>, StoreError>;
    fn accounts(&self, org: OrgId) -> Result<Vec<Account>, StoreError>;
    fn put_account(&self, account: Account) -> Result<(), StoreError>;
    fn put_token(&self, hash: String, token: Token) -> Result<(), StoreError>;
    fn token(&self, hash: &str) -> Result<Option<Token>, StoreError>;
    fn drop_token(&self, hash: &str) -> Result<(), StoreError>;
    fn revoke_tokens(&self, user: UserId) -> Result<(), StoreError>;
}

impl<S: AccountStore + ?Sized> AccountStore for std::sync::Arc<S> {
    fn account(&self, user: UserId) -> Result<Option<Account>, StoreError> {
        (**self).account(user)
    }
    fn account_named(&self, org: OrgId, name: &str) -> Result<Option<Account>, StoreError> {
        (**self).account_named(org, name)
    }
    fn accounts(&self, org: OrgId) -> Result<Vec<Account>, StoreError> {
        (**self).accounts(org)
    }
    fn put_account(&self, account: Account) -> Result<(), StoreError> {
        (**self).put_account(account)
    }
    fn put_token(&self, hash: String, token: Token) -> Result<(), StoreError> {
        (**self).put_token(hash, token)
    }
    fn token(&self, hash: &str) -> Result<Option<Token>, StoreError> {
        (**self).token(hash)
    }
    fn drop_token(&self, hash: &str) -> Result<(), StoreError> {
        (**self).drop_token(hash)
    }
    fn revoke_tokens(&self, user: UserId) -> Result<(), StoreError> {
        (**self).revoke_tokens(user)
    }
}

#[derive(Debug, Default)]
pub struct InMemoryAccounts {
    inner: Mutex<Tables>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Tables {
    #[serde(default)]
    pub(crate) accounts: Vec<Account>,
    #[serde(default)]
    pub(crate) tokens: HashMap<String, Token>,
}

impl Tables {
    pub(crate) fn account(&self, user: UserId) -> Option<Account> {
        self.accounts.iter().find(|account| account.user == user).cloned()
    }

    pub(crate) fn account_named(&self, org: OrgId, name: &str) -> Option<Account> {
        self.accounts.iter().find(|account| account.active && account.org == org && account.name == name).cloned()
    }

    pub(crate) fn accounts(&self, org: OrgId) -> Vec<Account> {
        self.accounts.iter().filter(|account| account.org == org).cloned().collect()
    }

    pub(crate) fn put_account(&mut self, account: Account) {
        match self.accounts.iter_mut().find(|kept| kept.user == account.user) {
            Some(kept) => *kept = account,
            None => self.accounts.push(account),
        }
    }

    pub(crate) fn revoke_tokens(&mut self, user: UserId) {
        self.tokens.retain(|_, token| token.user != user);
    }
}

impl InMemoryAccounts {
    fn tables(&self) -> MutexGuard<'_, Tables> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl AccountStore for InMemoryAccounts {
    fn account(&self, user: UserId) -> Result<Option<Account>, StoreError> {
        Ok(self.tables().account(user))
    }
    fn account_named(&self, org: OrgId, name: &str) -> Result<Option<Account>, StoreError> {
        Ok(self.tables().account_named(org, name))
    }
    fn accounts(&self, org: OrgId) -> Result<Vec<Account>, StoreError> {
        Ok(self.tables().accounts(org))
    }
    fn put_account(&self, account: Account) -> Result<(), StoreError> {
        self.tables().put_account(account);
        Ok(())
    }
    fn put_token(&self, hash: String, token: Token) -> Result<(), StoreError> {
        self.tables().tokens.insert(hash, token);
        Ok(())
    }
    fn token(&self, hash: &str) -> Result<Option<Token>, StoreError> {
        Ok(self.tables().tokens.get(hash).copied())
    }
    fn drop_token(&self, hash: &str) -> Result<(), StoreError> {
        self.tables().tokens.remove(hash);
        Ok(())
    }
    fn revoke_tokens(&self, user: UserId) -> Result<(), StoreError> {
        self.tables().revoke_tokens(user);
        Ok(())
    }
}
