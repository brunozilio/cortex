//! The accounts of channel plugins (ADR 0012): the numbers and mailboxes
//! the backend links itself.

use superfik_channels::Credentials;
use superfik_core::Principal;

use super::Plugins;
use crate::channels::ChannelAccount;

impl Plugins {
    pub fn channel_accounts(&self, owner: &Principal) -> Vec<ChannelAccount> {
        let book = self.store.load(owner);
        let present = self.present(owner, &book);
        book.connections
            .iter()
            .filter_map(|connection| {
                let kind = present
                    .iter()
                    .find(|p| p.package.manifest.id == connection.plugin)
                    .and_then(|p| p.package.manifest.channel)?;
                let credentials = Credentials::new(self.secrets.get(owner, connection.id).unwrap_or_default());
                Some(ChannelAccount { connection: connection.clone(), kind, credentials })
            })
            .collect()
    }
}
