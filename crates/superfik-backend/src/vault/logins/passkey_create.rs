//! Making a passkey for a page in the vault, as the Bitwarden extension
//! does on `navigator.credentials.create`: a new P-256 key kept on the
//! account's login, and the attestation (`none`) the page registers. The
//! private key never leaves the backend.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::SigningKey;
use p256::pkcs8::{EncodePrivateKey, EncodePublicKey};
use superfik_core::Principal;
use superfik_core::passkey::{
    Attestation, Ceremony, NewPasskey, PublicKey, attestation_object, authenticator_data, client_data_json,
    credential_id, rp_id_allowed,
};

use super::Logins;
use crate::vault::{KeptPasskey, VaultAccount, VaultPasskey};

impl Logins {
    pub fn create_passkey(
        &self,
        owner: &Principal,
        accounts: &[VaultAccount],
        new: &NewPasskey,
        created: &str,
    ) -> Result<Attestation, String> {
        if !rp_id_allowed(&new.rp_id, &new.origin) {
            return Err(format!("{} cannot make passkeys for {}", new.origin, new.rp_id));
        }
        let account = accounts
            .iter()
            .find(|a| a.title == new.account)
            .ok_or_else(|| format!("{} is not connected", new.account))?;
        let provider = self
            .providers
            .get(account.kind)
            .ok_or_else(|| format!("{}: this vault is not supported", account.title))?;
        let known = self.opened(owner, account, |session| provider.passkeys(account, session))?;
        let taken = known.iter().any(|p| {
            p.rp_id.eq_ignore_ascii_case(&new.rp_id)
                && credential_id(&p.credential).is_some_and(|id| new.excluded.contains(&id))
        });
        if taken {
            return Err(format!("The vault already holds a passkey for this {} account.", new.rp_id));
        }
        let (kept, attestation) = made(new, created)?;
        self.opened(owner, account, |session| provider.keep_passkey(account, session, &kept))?;
        Ok(attestation)
    }
}

fn made(new: &NewPasskey, created: &str) -> Result<(KeptPasskey, Attestation), String> {
    let key = fresh_key()?;
    let id = uuid::Uuid::new_v4();
    let point = key.verifying_key().to_sec1_point(false);
    let (Some(x), Some(y)) = (point.x(), point.y()) else { return Err("could not make a key".into()) };
    let public = PublicKey { x: (*x).into(), y: (*y).into() };
    let client_data = client_data_json(Ceremony::Create, &new.challenge, &new.origin);
    let data = authenticator_data(&new.rp_id, 0, Some((id.as_bytes(), &public)));
    let private = key.to_pkcs8_der().map_err(|_| "could not keep the key")?;
    let spki = key.verifying_key().to_public_key_der().map_err(|_| "could not describe the key")?;
    let encode = |bytes: &[u8]| URL_SAFE_NO_PAD.encode(bytes);
    let kept = KeptPasskey {
        passkey: VaultPasskey {
            item: String::new(),
            credential: id.to_string(),
            rp_id: new.rp_id.clone(),
            user_name: Some(new.user_name.clone()).filter(|n| !n.is_empty()),
            user_handle: Some(new.user_id.clone()),
            discoverable: true,
            key: encode(private.as_bytes()),
        },
        rp_name: new.rp_name.clone(),
        user_display_name: new.user_display_name.clone(),
        created: created.to_owned(),
    };
    let attestation = Attestation {
        credential_id: encode(id.as_bytes()),
        client_data: encode(client_data.as_bytes()),
        attestation_object: encode(&attestation_object(&data)),
        authenticator_data: encode(&data),
        public_key: encode(spki.as_bytes()),
    };
    Ok((kept, attestation))
}

fn fresh_key() -> Result<SigningKey, String> {
    for _ in 0..8 {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
        if let Ok(key) = SigningKey::from_slice(&bytes) {
            return Ok(key);
        }
    }
    Err("could not make a key".into())
}

#[cfg(test)]
mod tests;
