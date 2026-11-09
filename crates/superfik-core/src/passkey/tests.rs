use super::*;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn an_rp_id_is_the_origins_host_or_a_registrable_parent() {
    assert!(rp_id_allowed("google.com", "https://accounts.google.com"));
    assert!(rp_id_allowed("accounts.google.com", "https://accounts.google.com"));
    assert!(rp_id_allowed("localhost", "http://localhost:8080"));
    assert!(!rp_id_allowed("com", "https://google.com"), "a public suffix");
    assert!(!rp_id_allowed("co.uk", "https://bbc.co.uk"), "a public suffix");
    assert!(!rp_id_allowed("evil.com", "https://google.com"), "another site");
    assert!(!rp_id_allowed("ogle.com", "https://google.com"), "not a label boundary");
    assert!(!rp_id_allowed("google.com", "http://google.com"), "not secure");
    assert!(!rp_id_allowed("google.com", "about:blank"));
}

#[test]
fn client_data_names_the_ceremony_the_challenge_and_the_origin() {
    let json = client_data_json(Ceremony::Get, "Y2hhbGxlbmdl", "https://accounts.google.com");
    assert_eq!(
        json,
        r#"{"type":"webauthn.get","challenge":"Y2hhbGxlbmdl","origin":"https://accounts.google.com","crossOrigin":false}"#
    );
    assert!(client_data_json(Ceremony::Create, "x", "https://a.b").starts_with(r#"{"type":"webauthn.create""#));
}

#[test]
fn assertion_data_is_the_rp_hash_flags_and_counter() {
    let data = authenticator_data("localhost", 0, None);
    assert_eq!(hex(&data[..32]), "49960de5880e8c687434170f6476605b8fe4aeb9a28632c7995cf3ba831d9763");
    assert_eq!(data[32], 0x1d, "user present, verified, backup eligible and backed up");
    assert_eq!(data[33..], [0, 0, 0, 0]);
}

#[test]
fn registration_data_carries_the_credential_and_its_cose_key() {
    let key = PublicKey { x: [1; 32], y: [2; 32] };
    let data = authenticator_data("example.com", 0, Some((&[9, 9, 9], &key)));
    assert_eq!(hex(&data[..32]), "a379a6f6eeafb9a55e378c118034e2751e682fab9f2d30ab13d2125586ce1947");
    assert_eq!(data[32], 0x5d, "attested credential data included");
    assert_eq!(data[37..53], [0; 16], "no AAGUID");
    assert_eq!(data[53..55], [0, 3]);
    assert_eq!(data[55..58], [9, 9, 9]);
    let cose = &data[58..];
    assert_eq!(cose[..10], [0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21, 0x58, 0x20]);
    assert_eq!(cose[10..42], [1; 32]);
    assert_eq!(cose[42..45], [0x22, 0x58, 0x20]);
    assert_eq!(cose[45..], [2; 32]);
}

#[test]
fn attestation_is_none_around_the_authenticator_data() {
    let object = attestation_object(&[7; 300]);
    let head = [
        &[0xa3, 0x63][..],
        b"fmt",
        &[0x64],
        b"none",
        &[0x67],
        b"attStmt",
        &[0xa0, 0x68],
        b"authData",
        &[0x59, 0x01, 0x2c],
    ]
    .concat();
    assert_eq!(object[..head.len()], head[..]);
    assert_eq!(object.len(), head.len() + 300);
    assert_eq!(attestation_object(&[7; 30])[head.len() - 3..head.len() - 1], [0x58, 30]);
}

#[test]
fn bitwarden_credential_ids_become_their_raw_bytes() {
    assert_eq!(
        credential_bytes("b5b9b2a4-4c26-4b8d-9f6e-1f0a2b3c4d5e").map(|b| hex(&b)),
        Some("b5b9b2a44c264b8d9f6e1f0a2b3c4d5e".into())
    );
    assert_eq!(credential_bytes("b64.AQID"), Some(vec![1, 2, 3]));
    assert_eq!(credential_bytes("nonsense"), None);
    assert_eq!(credential_id("b5b9b2a4-4c26-4b8d-9f6e-1f0a2b3c4d5e").as_deref(), Some("tbmypEwmS42fbh8KKzxNXg"));
}

fn stored(id: &str, rp: &str, discoverable: bool) -> StoredPasskey {
    StoredPasskey {
        item: "Google".into(),
        account: "Bitwarden · Pessoal".into(),
        credential: id.into(),
        rp_id: rp.into(),
        user_name: Some("ana@example.com".into()),
        discoverable,
    }
}

#[test]
fn a_request_offers_the_passkeys_of_its_rp_allowed_or_discoverable() {
    let a = stored("b64.AQID", "google.com", true);
    let b = stored("b64.BAUG", "google.com", false);
    let other = stored("b64.BwgJ", "github.com", true);
    let all = [a.clone(), b.clone(), other];
    assert_eq!(offered(&all, "google.com", &[]), std::slice::from_ref(&a), "only discoverable without a list");
    assert_eq!(offered(&all, "google.com", &["BAUG".into()]), [b], "the allowed one");
    assert_eq!(offered(&all, "Google.COM", &["AQID".into(), "BwgJ".into()]), [a], "another rp's never");
}
