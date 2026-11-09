use std::sync::{Arc, Mutex};

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{DerSignature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use sha2::{Digest, Sha256};
use superfik_core::VaultKind;
use superfik_core::passkey::{PasskeyUse, decode};

use super::*;
use crate::vault::{Item, VaultProvider, VaultProviders};

#[derive(Default)]
struct Keeps(Mutex<Vec<VaultPasskey>>);

impl VaultProvider for Keeps {
    fn items(&self, _: &VaultAccount, _: &mut Option<String>) -> Result<Vec<Item>, String> {
        Ok(Vec::new())
    }

    fn passkeys(&self, _: &VaultAccount, _: &mut Option<String>) -> Result<Vec<VaultPasskey>, String> {
        Ok(self.0.lock().expect("kept").clone())
    }

    fn keep_passkey(&self, _: &VaultAccount, _: &mut Option<String>, new: &KeptPasskey) -> Result<(), String> {
        self.0.lock().expect("kept").push(VaultPasskey { item: "GitHub".into(), ..new.passkey.clone() });
        Ok(())
    }
}

fn account() -> VaultAccount {
    VaultAccount {
        title: "Bitwarden · Pessoal".into(),
        kind: VaultKind::Bitwarden,
        scope: "Superfik".into(),
        secrets: crate::plugins::Secrets::default(),
    }
}

fn new(origin: &str) -> NewPasskey {
    NewPasskey {
        origin: origin.into(),
        rp_id: "github.com".into(),
        rp_name: "GitHub".into(),
        challenge: "Y2g".into(),
        user_id: "dXNlcg".into(),
        user_name: "bruno".into(),
        user_display_name: "Bruno".into(),
        excluded: Vec::new(),
        account: "Bitwarden · Pessoal".into(),
    }
}

const OWNER: Principal = Principal::LocalOwner;
const NOW: &str = "2026-10-31T22:00:00.000Z";

#[test]
fn a_new_passkey_is_kept_and_signs_in_with_the_key_the_page_registered() {
    let vault = Arc::new(Keeps::default());
    let logins = Logins::new(VaultProviders::default().with(VaultKind::Bitwarden, vault.clone()));
    let accounts = [account()];
    let made = logins.create_passkey(&OWNER, &accounts, &new("https://github.com"), NOW).expect("made");
    let client_data = String::from_utf8(decode(&made.client_data).expect("b64")).expect("utf8");
    assert!(client_data.starts_with(r#"{"type":"webauthn.create","challenge":"Y2g","origin":"https://github.com""#));
    let data = decode(&made.authenticator_data).expect("b64");
    assert_eq!(data[32], 0x5d, "attested, verified, backed up");
    assert_eq!(decode(&made.attestation_object).expect("b64")[..2], [0xa3, 0x63]);
    let kept = vault.0.lock().expect("kept").clone();
    assert_eq!(kept.len(), 1);
    assert_eq!(credential_id(&kept[0].credential).as_deref(), Some(made.credential_id.as_str()));
    assert_eq!(kept[0].user_handle.as_deref(), Some("dXNlcg"));

    let chosen = PasskeyUse {
        origin: "https://github.com".into(),
        rp_id: "github.com".into(),
        challenge: "c2lnbg".into(),
        account: "Bitwarden · Pessoal".into(),
        credential: kept[0].credential.clone(),
    };
    let signed = logins.sign_passkey(&OWNER, &accounts, &chosen).expect("signs");
    let public = VerifyingKey::from_public_key_der(&decode(&made.public_key).expect("b64")).expect("spki");
    let message = [
        decode(&signed.authenticator_data).expect("b64"),
        Sha256::digest(decode(&signed.client_data).expect("b64")).to_vec(),
    ]
    .concat();
    let signature = DerSignature::try_from(decode(&signed.signature).expect("b64").as_slice()).expect("der");
    public.verify(&message, &signature).expect("the registered key verifies the sign-in");
}

#[test]
fn nothing_is_made_for_another_site_or_twice_for_one_account() {
    let vault = Arc::new(Keeps::default());
    let logins = Logins::new(VaultProviders::default().with(VaultKind::Bitwarden, vault.clone()));
    let accounts = [account()];
    assert!(logins.create_passkey(&OWNER, &accounts, &new("https://evil.com"), NOW).is_err());
    let made = logins.create_passkey(&OWNER, &accounts, &new("https://github.com"), NOW).expect("made");
    let again = NewPasskey { excluded: vec![made.credential_id], ..new("https://github.com") };
    assert!(logins.create_passkey(&OWNER, &accounts, &again, NOW).is_err_and(|e| e.contains("already")));
    assert_eq!(vault.0.lock().expect("kept").len(), 1);
}
