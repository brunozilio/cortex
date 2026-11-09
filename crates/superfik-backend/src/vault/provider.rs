//! The seam between the vault tools and a kind of vault. A provider only
//! lists one scope's items with their values; asking, running, hiding
//! values and the MCP tools are the same for every kind. Adding a vault is
//! a [`VaultKind`] variant, a [`VaultProvider`] and a catalog manifest.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use superfik_core::VaultKind;
use superfik_core::autofill::{LoginUri, NewLogin};

use super::bitwarden::{Bitwarden, SystemBitwarden};
use crate::plugins::Secrets;

#[derive(Clone)]
pub struct VaultAccount {
    pub title: String,
    pub kind: VaultKind,
    pub scope: String,
    pub secrets: Secrets,
}

impl std::fmt::Debug for VaultAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultAccount").field("title", &self.title).field("scope", &self.scope).finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub values: BTreeMap<String, String>,
    pub uris: Vec<LoginUri>,
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("name", &self.name)
            .field("fields", &self.values.keys())
            .field("uris", &self.uris)
            .finish()
    }
}

pub trait VaultProvider: Send + Sync {
    fn items(&self, account: &VaultAccount, session: &mut Option<String>) -> Result<Vec<Item>, String>;

    fn logins(&self, account: &VaultAccount, session: &mut Option<String>, _url: &str) -> Result<Vec<Item>, String> {
        self.items(account, session)
    }

    fn refresh(&self, _account: &VaultAccount, _session: &str) -> Result<(), String> {
        Ok(())
    }

    fn save(&self, account: &VaultAccount, _session: &mut Option<String>, _login: &NewLogin) -> Result<(), String> {
        Err(format!("{} cannot keep new logins", account.kind.name()))
    }

    fn passkeys(&self, _account: &VaultAccount, _session: &mut Option<String>) -> Result<Vec<VaultPasskey>, String> {
        Ok(Vec::new())
    }

    fn keep_passkey(
        &self,
        account: &VaultAccount,
        _session: &mut Option<String>,
        _new: &KeptPasskey,
    ) -> Result<(), String> {
        Err(format!("{} cannot keep passkeys", account.kind.name()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptPasskey {
    pub passkey: VaultPasskey,
    pub rp_name: String,
    pub user_display_name: String,
    pub created: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct VaultPasskey {
    pub item: String,
    pub credential: String,
    pub rp_id: String,
    pub user_name: Option<String>,
    pub user_handle: Option<String>,
    pub discoverable: bool,
    pub key: String,
}

impl std::fmt::Debug for VaultPasskey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultPasskey")
            .field("item", &self.item)
            .field("credential", &self.credential)
            .field("rp_id", &self.rp_id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
pub struct VaultProviders(HashMap<VaultKind, Arc<dyn VaultProvider>>);

impl std::fmt::Debug for VaultProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

impl VaultProviders {
    pub fn system() -> Self {
        Self::default().with(VaultKind::Bitwarden, Arc::new(Bitwarden::new(Arc::new(SystemBitwarden))))
    }

    pub fn with(mut self, kind: VaultKind, provider: Arc<dyn VaultProvider>) -> Self {
        self.0.insert(kind, provider);
        self
    }

    pub fn get(&self, kind: VaultKind) -> Option<&Arc<dyn VaultProvider>> {
        self.0.get(&kind)
    }
}
