use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{DerSignature, VerifyingKey};
use p256::pkcs8::EncodePrivateKey;
use superfik_core::VaultKind;

use super::*;
use crate::vault::{Item, VaultProvider, VaultProviders};

fn key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32].into()).expect("a key")
}

struct Passkeys;

impl VaultProvider for Passkeys {
    fn items(&self, _: &VaultAccount, _: &mut Option<String>) -> Result<Vec<Item>, String> {
        Ok(Vec::new())
    }

    fn passkeys(&self, account: &VaultAccount, _: &mut Option<String>) -> Result<Vec<VaultPasskey>, String> {
        if account.secrets.get("password").map(String::as_str) != Some("hunter2") {
            return Err("could not unlock".into());
        }
        let der = key().to_pkcs8_der().expect("pkcs8");
        Ok(vec![VaultPasskey {
            item: "Google".into(),
            credential: "b64.AQID".into(),
            rp_id: "google.com".into(),
            user_name: Some("ana@example.com".into()),
            user_handle: Some("dXNlcg".into()),
            discoverable: true,
            key: STANDARD.encode(der.as_bytes()),
        }])
    }
}

fn account(title: &str, password: &str) -> VaultAccount {
    VaultAccount {
        title: title.into(),
        kind: VaultKind::Bitwarden,
        scope: "Superfik".into(),
        secrets: crate::plugins::Secrets::from([("password".to_owned(), password.to_owned())]),
    }
}

fn logins() -> Logins {
    Logins::new(VaultProviders::default().with(VaultKind::Bitwarden, Arc::new(Passkeys)))
}

const OWNER: Principal = Principal::LocalOwner;

fn chosen(origin: &str, rp_id: &str) -> PasskeyUse {
    PasskeyUse {
        origin: origin.into(),
        rp_id: rp_id.into(),
        challenge: "Y2hhbGxlbmdl".into(),
        account: "Bitwarden · Pessoal".into(),
        credential: "b64.AQID".into(),
    }
}

#[test]
fn the_page_sees_its_passkeys_once_without_their_keys() {
    let both = [account("Bitwarden · Pessoal", "hunter2"), account("Bitwarden · Work", "hunter2")];
    let found = logins().passkeys(&OWNER, &both, "google.com", &[]).expect("found");
    assert_eq!(found.len(), 1, "two accounts of one vault list it once");
    assert_eq!((found[0].item.as_str(), found[0].account.as_str()), ("Google", "Bitwarden · Pessoal"));
    assert!(logins().passkeys(&OWNER, &both, "github.com", &[]).expect("none").is_empty());
    let locked = [account("Bitwarden · Pessoal", "wrong")];
    assert!(logins().passkeys(&OWNER, &locked, "google.com", &[]).is_err());
}

#[test]
fn a_signature_verifies_against_the_passkeys_public_key() {
    let accounts = [account("Bitwarden · Pessoal", "hunter2")];
    let assertion =
        logins().sign_passkey(&OWNER, &accounts, &chosen("https://accounts.google.com", "google.com")).expect("signs");
    let bytes = |text: &str| decode(text).expect("base64url");
    let client_data = String::from_utf8(bytes(&assertion.client_data)).expect("utf8");
    assert!(client_data.contains(r#""origin":"https://accounts.google.com""#), "{client_data}");
    assert!(client_data.contains(r#""challenge":"Y2hhbGxlbmdl""#));
    let signed = [bytes(&assertion.authenticator_data), Sha256::digest(client_data.as_bytes()).to_vec()].concat();
    let signature = DerSignature::try_from(bytes(&assertion.signature).as_slice()).expect("der");
    VerifyingKey::from(&key()).verify(&signed, &signature).expect("verifies");
    assert_eq!(assertion.credential_id, "AQID");
    assert_eq!(assertion.user_handle.as_deref(), Some("dXNlcg"));
}

#[test]
fn nothing_is_signed_for_another_site_or_a_passkey_gone() {
    let accounts = [account("Bitwarden · Pessoal", "hunter2")];
    let elsewhere = logins().sign_passkey(&OWNER, &accounts, &chosen("https://evil.com", "google.com"));
    assert!(elsewhere.is_err_and(|e| e.contains("cannot ask")));
    let unknown = PasskeyUse { credential: "b64.BAUG".into(), ..chosen("https://google.com", "google.com") };
    assert!(logins().sign_passkey(&OWNER, &accounts, &unknown).is_err());
    let other = PasskeyUse { account: "Bitwarden · Other".into(), ..chosen("https://google.com", "google.com") };
    assert!(logins().sign_passkey(&OWNER, &accounts, &other).is_err());
}
