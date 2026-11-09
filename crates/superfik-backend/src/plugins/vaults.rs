//! The vault accounts a session picked, with what unlocks them. Each
//! account opens one scope (a folder, a vault): its label, or
//! [`DEFAULT_SCOPE`] without one.

use superfik_core::{Connection, Principal};

use super::Plugins;
use crate::vault::VaultAccount;

pub const DEFAULT_SCOPE: &str = "Superfik";

impl Plugins {
    pub fn vaults(&self, owner: &Principal, wanted: &[Connection]) -> Result<Vec<VaultAccount>, String> {
        let book = self.store.load(owner);
        let present = self.present(owner, &book);
        let mut accounts = Vec::new();
        for connection in wanted.iter().filter_map(|w| book.connection(w.id)) {
            let Some(kind) = present
                .iter()
                .find(|p| p.package.manifest.id == connection.plugin)
                .and_then(|p| p.package.manifest.vault)
            else {
                continue;
            };
            let secrets = self
                .secrets
                .get(owner, connection.id)
                .filter(|secrets| secrets.contains_key(kind.unlock_secret()))
                .ok_or_else(|| format!("{} is signed out; connect it again in Plugins", connection.title()))?;
            let scope = connection.label.trim();
            accounts.push(VaultAccount {
                title: connection.title(),
                kind,
                scope: if scope.is_empty() { DEFAULT_SCOPE } else { scope }.to_owned(),
                secrets,
            });
        }
        Ok(accounts)
    }

    pub fn all_vaults(&self, owner: &Principal) -> Result<Vec<VaultAccount>, String> {
        let connections = self.store.load(owner).connections;
        self.vaults(owner, &connections)
    }
}
