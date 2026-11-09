//! The server's users and what can be done to them: adding one with a
//! first-access code, a new code, roles and removal.

use superfik_core::access::OrgRole;
use superfik_core::{Member, OrgId, Principal, UserId};

use crate::secret::{code_hash, first_access_code, normalized_code};
use crate::store::{Account, AccountStore, FirstAccess, StoreError};

const DAY: u64 = 86_400;
pub const CODE_LIFETIME: u64 = 7 * DAY;
pub const TOKEN_LIFETIME: u64 = 90 * DAY;
const MAX_NAME: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    #[error("not allowed")]
    Forbidden,
    #[error("usernames are 1 to 32 letters, digits, dots, dashes or underscores")]
    BadName,
    #[error("someone on this server already uses that username")]
    NameTaken,
    #[error("wrong username or first-access code, or the code expired")]
    WrongCode,
    #[error("passwords need at least {} characters", crate::MIN_PASSWORD)]
    WeakPassword,
    #[error("wrong username or password")]
    WrongCredentials,
    #[error("this user has not made their first access yet")]
    FirstAccessPending,
    #[error("too many tries; wait a few minutes")]
    TooManyAttempts,
    #[error("the server needs at least one owner")]
    LastOwner,
    #[error("no such user")]
    Unknown,
    #[error("{0}")]
    Store(String),
}

impl From<StoreError> for IdentityError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.0)
    }
}

pub(crate) type Result<T> = std::result::Result<T, IdentityError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Console,
    Member(Member),
}

#[derive(Debug)]
pub struct Directory<S> {
    pub(crate) org: OrgId,
    pub(crate) store: S,
    pub(crate) attempts: std::sync::Mutex<std::collections::HashMap<String, crate::sign_in::Attempts>>,
}

pub fn username(name: &str) -> Result<String> {
    let name = name.trim().to_lowercase();
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    if name.is_empty() || name.len() > MAX_NAME || !name.chars().all(allowed) {
        return Err(IdentityError::BadName);
    }
    Ok(name)
}

impl<S: AccountStore> Directory<S> {
    pub fn new(org: OrgId, store: S) -> Self {
        Self { org, store, attempts: std::sync::Mutex::default() }
    }

    pub fn org(&self) -> OrgId {
        self.org
    }

    pub fn add(&self, by: Actor, name: &str, role: OrgRole, now: u64) -> Result<(Member, String)> {
        self.may_grant(by, role)?;
        let name = username(name)?;
        if self.store.account_named(self.org, &name)?.is_some() {
            return Err(IdentityError::NameTaken);
        }
        let (first_access, code) = fresh_code(now)?;
        let user = UserId::new();
        let account =
            Account { user, org: self.org, name, role, password: None, first_access: Some(first_access), active: true };
        self.store.put_account(account)?;
        Ok((Member { user, org: self.org }, code))
    }

    pub fn new_code(&self, by: Actor, user: UserId, now: u64) -> Result<String> {
        let mut account = self.managed(by, user)?;
        let (first_access, code) = fresh_code(now)?;
        account.password = None;
        account.first_access = Some(first_access);
        self.store.put_account(account)?;
        self.store.revoke_tokens(user)?;
        Ok(code)
    }

    pub fn authenticate(&self, token: &str, now: u64) -> Option<Principal> {
        let token = self.store.token(&code_hash(token)).ok()??;
        let account = self.store.account(token.user).ok()??;
        (now < token.expires_unix && account.active && account.org == self.org && !account.pending())
            .then(|| Principal::member(account.user, account.org))
    }

    pub fn role(&self, member: &Member) -> Option<OrgRole> {
        if member.org != self.org {
            return None;
        }
        self.store.account(member.user).ok()?.filter(|account| account.active).map(|account| account.role)
    }

    pub fn members(&self) -> Result<Vec<Account>> {
        let mut members: Vec<Account> =
            self.store.accounts(self.org)?.into_iter().filter(|account| account.active).collect();
        members.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(members)
    }

    pub fn named(&self, name: &str) -> Result<Account> {
        self.store.account_named(self.org, &username(name)?)?.ok_or(IdentityError::Unknown)
    }

    pub fn has_owner(&self) -> Result<bool> {
        Ok(self.members()?.iter().any(|account| account.role == OrgRole::Owner))
    }

    pub fn set_role(&self, by: Actor, user: UserId, role: OrgRole) -> Result<()> {
        let mut account = self.managed(by, user)?;
        self.may_grant(by, role)?;
        if account.role == OrgRole::Owner && role != OrgRole::Owner {
            self.keep_an_owner()?;
        }
        account.role = role;
        Ok(self.store.put_account(account)?)
    }

    pub fn remove(&self, by: Actor, user: UserId) -> Result<()> {
        let mut account = self.managed(by, user)?;
        if account.role == OrgRole::Owner {
            self.keep_an_owner()?;
        }
        account.active = false;
        account.first_access = None;
        self.store.put_account(account)?;
        Ok(self.store.revoke_tokens(user)?)
    }

    fn may_grant(&self, by: Actor, role: OrgRole) -> Result<()> {
        let Actor::Member(member) = by else { return Ok(()) };
        match (self.role(&member), role) {
            (Some(OrgRole::Owner), _) => Ok(()),
            (Some(OrgRole::Admin), role) if role != OrgRole::Owner => Ok(()),
            _ => Err(IdentityError::Forbidden),
        }
    }

    fn managed(&self, by: Actor, user: UserId) -> Result<Account> {
        let account = self
            .store
            .account(user)?
            .filter(|account| account.active && account.org == self.org)
            .ok_or(IdentityError::Unknown)?;
        let Actor::Member(member) = by else { return Ok(account) };
        match (self.role(&member), account.role) {
            (Some(OrgRole::Owner), _) => Ok(account),
            (Some(OrgRole::Admin), role) if role != OrgRole::Owner => Ok(account),
            _ => Err(IdentityError::Forbidden),
        }
    }

    fn keep_an_owner(&self) -> Result<()> {
        let owners = self.members()?.iter().filter(|account| account.role == OrgRole::Owner).count();
        if owners > 1 { Ok(()) } else { Err(IdentityError::LastOwner) }
    }

    pub(crate) fn by_code(&self, name: &str, code: &str, now: u64) -> Result<Account> {
        let hash = code_hash(&normalized_code(code));
        username(name)
            .ok()
            .and_then(|name| self.store.account_named(self.org, &name).ok().flatten())
            .filter(|account| {
                account.first_access.as_ref().is_some_and(|first| first.code_hash == hash && now < first.expires_unix)
            })
            .ok_or(IdentityError::WrongCode)
    }
}

fn fresh_code(now: u64) -> Result<(FirstAccess, String)> {
    let code = first_access_code().ok_or_else(|| IdentityError::Store("no randomness".to_owned()))?;
    let kept = FirstAccess { code_hash: code_hash(&normalized_code(&code)), expires_unix: now + CODE_LIFETIME };
    Ok((kept, code))
}
