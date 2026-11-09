//! Vault logins for the browser card. The user's own fill picks from every
//! vault account they connected and needs no ask; an agent's (`run`) only
//! from the accounts its session picked, once the user allows it on the
//! ask card, on a thread of its own since the answer can take minutes.

use superfik_core::autofill::sign_in_subject;
use superfik_core::{BrowserId, Principal, RunId};
use superfik_protocol::ServerEvent;

use crate::outbox::Outbox;
use crate::plugins::Plugins;
use crate::vault::{Logins, Vault};

pub fn find(plugins: &Plugins, logins: &Logins, owner: &Principal, browser: BrowserId, url: String) -> ServerEvent {
    let candidates = match plugins.all_vaults(owner).and_then(|accounts| logins.candidates(owner, &accounts, &url)) {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::warn!(%error, "could not look up the page's vault logins");
            Vec::new()
        }
    };
    ServerEvent::LoginCandidates { browser, url, candidates }
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub browser: BrowserId,
    pub url: String,
    pub account: String,
    pub item: String,
    pub run: Option<RunId>,
}

pub fn fill(plugins: &Plugins, logins: &Logins, vault: Option<&Vault>, outbox: &Outbox, owner: &Principal, fill: Fill) {
    let Some(run) = fill.run else {
        let accounts = plugins.all_vaults(owner);
        outbox.send(answer(logins, owner, accounts, fill));
        return;
    };
    let Some(vault) = vault.cloned() else {
        let message = "The vault tools are unavailable.".to_owned();
        return outbox.send(ServerEvent::LoginFailed { browser: fill.browser, message });
    };
    let (logins, outbox, owner) = (logins.clone(), outbox.clone(), owner.clone());
    let asking = std::thread::Builder::new().name("superfik-sign-in".into()).spawn(move || {
        let accounts = vault.approve(&owner, run, &fill.item, sign_in_subject(&fill.item, &fill.url));
        outbox.send(answer(&logins, &owner, accounts, fill));
    });
    if let Err(error) = asking {
        tracing::warn!(%error, "could not ask for a sign-in");
    }
}

fn answer(
    logins: &Logins,
    owner: &Principal,
    accounts: Result<Vec<crate::vault::VaultAccount>, String>,
    fill: Fill,
) -> ServerEvent {
    let Fill { browser, url, account, item, run } = fill;
    let scoped = run.is_some();
    match accounts.and_then(|accounts| logins.fill(owner, &accounts, &url, &account, &item, scoped)) {
        Ok(fill) => ServerEvent::LoginSecret { browser, url, fill },
        Err(message) => ServerEvent::LoginFailed { browser, message },
    }
}

pub fn save(
    plugins: &Plugins,
    logins: &Logins,
    owner: &Principal,
    browser: BrowserId,
    account: String,
    login: &superfik_core::autofill::NewLogin,
) -> ServerEvent {
    match plugins.all_vaults(owner).and_then(|accounts| logins.save(owner, &accounts, &account, login)) {
        Ok(()) => ServerEvent::LoginSaved { browser, account },
        Err(message) => ServerEvent::LoginFailed { browser, message },
    }
}

pub fn find_passkeys(
    plugins: &Plugins,
    logins: &Logins,
    owner: &Principal,
    browser: BrowserId,
    rp_id: String,
    allowed: &[String],
) -> ServerEvent {
    let found = plugins.all_vaults(owner).and_then(|accounts| logins.passkeys(owner, &accounts, &rp_id, allowed));
    match found {
        Ok(passkeys) => ServerEvent::PasskeyCandidates { browser, rp_id, passkeys },
        Err(message) => ServerEvent::PasskeyFailed { browser, message },
    }
}

pub fn sign_passkey(
    plugins: &Plugins,
    logins: &Logins,
    owner: &Principal,
    browser: BrowserId,
    chosen: &superfik_core::passkey::PasskeyUse,
) -> ServerEvent {
    match plugins.all_vaults(owner).and_then(|accounts| logins.sign_passkey(owner, &accounts, chosen)) {
        Ok(assertion) => ServerEvent::PasskeySigned { browser, assertion },
        Err(message) => ServerEvent::PasskeyFailed { browser, message },
    }
}

pub fn create_passkey(
    plugins: &Plugins,
    logins: &Logins,
    owner: &Principal,
    browser: BrowserId,
    new: &superfik_core::passkey::NewPasskey,
) -> ServerEvent {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    match plugins.all_vaults(owner).and_then(|accounts| logins.create_passkey(owner, &accounts, new, &now)) {
        Ok(attestation) => ServerEvent::PasskeyCreated { browser, attestation },
        Err(message) => ServerEvent::PasskeyFailed { browser, message },
    }
}
