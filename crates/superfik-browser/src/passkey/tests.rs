use super::*;

#[test]
fn a_request_reads_its_relying_party_or_the_origins_host() {
    let ask = PasskeyAsk::read(
        r#"{"request":3,"kind":"get","rpId":"google.com","challenge":"Y2g","allowed":["AQID"]}"#,
        "https://accounts.google.com",
    )
    .expect("read");
    assert_eq!((ask.request, ask.rp_id.as_str(), ask.allowed.as_slice()), (3, "google.com", &["AQID".to_owned()][..]));
    assert_eq!(ask.origin, "https://accounts.google.com", "from the context, not the script");
    assert!(!ask.conditional && ask.registration.is_none());
    let bare = PasskeyAsk::read(r#"{"request":1,"rpId":"","challenge":"Y2g"}"#, "https://github.com").expect("read");
    assert_eq!(bare.rp_id, "github.com");
    assert!(PasskeyAsk::read("nonsense", "https://github.com").is_none());
}

#[test]
fn a_conditional_request_says_so() {
    let ask = PasskeyAsk::read(r#"{"request":1,"kind":"get","conditional":true,"challenge":"Y2g"}"#, "https://a.com")
        .expect("read");
    assert!(ask.conditional);
}

#[test]
fn a_registration_names_the_account_and_what_the_page_knows() {
    let payload = r#"{"request":2,"kind":"create","rpId":"github.com","rpName":"GitHub","challenge":"Y2g",
        "user":{"id":"dXNlcg","name":"bruno","displayName":"Bruno"},"excluded":["AQID"],"algorithms":[-8,-7]}"#;
    let ask = PasskeyAsk::read(payload, "https://github.com").expect("read");
    let registration = ask.registration.expect("create");
    assert_eq!(
        (registration.rp_name.as_str(), registration.user_id.as_str(), registration.user_name.as_str()),
        ("GitHub", "dXNlcg", "bruno")
    );
    assert!(registration.es256);
    assert_eq!(ask.allowed, ["AQID"], "the ids it already knows");
    let rsa = payload.replace("[-8,-7]", "[-257]");
    assert!(!PasskeyAsk::read(&rsa, "https://github.com").and_then(|a| a.registration).expect("create").es256);
    assert!(PasskeyAsk::read(r#"{"request":2,"kind":"create","challenge":"Y2g"}"#, "https://a.com").is_none());
}

#[test]
fn the_script_hides_its_binding_and_keeps_the_original_calls() {
    let script = shim_script("__sfk_ab12");
    assert!(script.contains(&format!("delete window.{BINDING}")));
    assert!(script.contains("return get(options)") && script.contains("return create(options)"));
    assert!(script.contains("Object.defineProperty(window, '__sfk_ab12'"));
    assert!(!script.contains("__ANSWER__") && !script.contains("__BINDING__"));
}

#[test]
fn answers_name_the_request() {
    assert_eq!(answer_script("f", 2, &PasskeyReply::Elsewhere), r#"window.f && window.f(2, {"elsewhere":true})"#);
    assert_eq!(answer_script("f", 2, &PasskeyReply::Refused), "window.f && window.f(2, {})");
    let signed = Assertion {
        credential_id: "AQID".into(),
        client_data: "e30".into(),
        authenticator_data: "AA".into(),
        signature: "MEU".into(),
        user_handle: None,
    };
    let script = answer_script("f", 2, &PasskeyReply::Signed(signed));
    assert!(script.contains(r#"{"signed":{"#) && script.contains(r#""credential_id":"AQID""#), "{script}");
    let created = Attestation {
        credential_id: "AQID".into(),
        client_data: "e30".into(),
        attestation_object: "o2M".into(),
        authenticator_data: "AA".into(),
        public_key: "MFk".into(),
    };
    assert!(answer_script("f", 3, &PasskeyReply::Created(created)).contains(r#"{"created":{"#));
}
