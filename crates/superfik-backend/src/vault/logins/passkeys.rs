//! Passkeys for the browser card, as the Bitwarden extension answers a
//! page: the ones kept for the page's relying party, then a signature by
//! the one the user picks. The private key never leaves the backend; the
//! origin is checked against the relying party before anything is signed.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{DerSignature, SigningKey};
use p256::pkcs8::DecodePrivateKey;
use sha2::{Digest, Sha256};
use superfik_core::Principal;
use superfik_core::passkey::{
    Assertion, Ceremony, PasskeyUse, StoredPasskey, authenticator_data, client_data_json, credential_id, decode,
    offered, rp_id_allowed,
};

use super::Logins;
use crate::vault::{VaultAccount, VaultPasskey};

impl Logins {
    pub fn passkeys(
        &self,
        owner: &Principal,
        accounts: &[VaultAccount],
        rp_id: &str,
        allowed: &[String],
    ) -> Result<Vec<StoredPasskey>, String> {
        let mut stored: Vec<StoredPasskey> = Vec::new();
        let mut problems = Vec::new();
        for account in accounts {
            match self.vault_passkeys(owner, account) {
                Ok(found) => {
                    for passkey in found {
                        if !stored.iter().any(|s| s.credential == passkey.credential) {
                            stored.push(described(&passkey, &account.title));
                        }
                    }
                }
                Err(error) => problems.push(error),
            }
        }
        if stored.is_empty() && !problems.is_empty() {
            return Err(problems.join("\n"));
        }
        Ok(offered(&stored, rp_id, allowed))
    }

    pub fn sign_passkey(
        &self,
        owner: &Principal,
        accounts: &[VaultAccount],
        chosen: &PasskeyUse,
    ) -> Result<Assertion, String> {
        if !rp_id_allowed(&chosen.rp_id, &chosen.origin) {
            return Err(format!("{} cannot ask for passkeys of {}", chosen.origin, chosen.rp_id));
        }
        let account = accounts
            .iter()
            .find(|a| a.title == chosen.account)
            .ok_or_else(|| format!("{} is not connected", chosen.account))?;
        let found = self.vault_passkeys(owner, account)?;
        let passkey = found
            .iter()
            .find(|p| p.credential == chosen.credential && p.rp_id.eq_ignore_ascii_case(&chosen.rp_id))
            .ok_or("this passkey is no longer in the vault")?;
        sign(passkey, chosen)
    }

    fn vault_passkeys(&self, owner: &Principal, account: &VaultAccount) -> Result<Vec<VaultPasskey>, String> {
        let provider = self
            .providers
            .get(account.kind)
            .ok_or_else(|| format!("{}: this vault is not supported", account.title))?;
        self.opened(owner, account, |session| provider.passkeys(account, session))
    }
}

fn described(passkey: &VaultPasskey, account: &str) -> StoredPasskey {
    StoredPasskey {
        item: passkey.item.clone(),
        account: account.to_owned(),
        credential: passkey.credential.clone(),
        rp_id: passkey.rp_id.clone(),
        user_name: passkey.user_name.clone(),
        discoverable: passkey.discoverable,
    }
}

fn sign(passkey: &VaultPasskey, chosen: &PasskeyUse) -> Result<Assertion, String> {
    let der = decode(&passkey.key).ok_or("the passkey's key is unreadable")?;
    let key = SigningKey::from_pkcs8_der(&der).map_err(|_| "the passkey's key is unreadable")?;
    let client_data = client_data_json(Ceremony::Get, &chosen.challenge, &chosen.origin);
    let data = authenticator_data(&chosen.rp_id, 0, None);
    let signed = [data.as_slice(), &Sha256::digest(client_data.as_bytes())].concat();
    let signature: DerSignature = key.sign(&signed);
    let encode = |bytes: &[u8]| URL_SAFE_NO_PAD.encode(bytes);
    Ok(Assertion {
        credential_id: credential_id(&passkey.credential).ok_or("the passkey's id is unreadable")?,
        client_data: encode(client_data.as_bytes()),
        authenticator_data: encode(&data),
        signature: encode(signature.as_bytes()),
        user_handle: passkey.user_handle.as_deref().and_then(decode).map(|h| encode(&h)),
    })
}

#[cfg(test)]
mod tests;
