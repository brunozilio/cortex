//! Vault passkeys in the page: a real `navigator.credentials.get` reaches
//! the host as `PageEvent::PasskeyRequested` with the page's own origin,
//! and the host's answers come back as a `PublicKeyCredential` or a
//! `NotAllowedError`. The page cannot reach the binding itself.
use std::time::{Duration, Instant};

use superfik_browser::passkey::{PasskeyAsk, PasskeyReply};
use superfik_browser::{BrowserHost, Call, PageEvent};
use superfik_core::passkey::{Assertion, Attestation};
use winit::platform::pump_events::EventLoopExtPumpEvents;

use super::{Error, Harness};

const ASK: &str = "window.__passkey = 'waiting'; navigator.credentials.get({ publicKey: { challenge: new Uint8Array([1, 2, 3]), \
     allowCredentials: [{ type: 'public-key', id: new Uint8Array([4, 5, 6]) }] } }).then(\
     c => window.__passkey = [c instanceof PublicKeyCredential, c.id, c.type, new Uint8Array(c.response.signature).length, \
     new TextDecoder().decode(c.response.clientDataJSON), c.authenticatorAttachment, JSON.stringify(c.toJSON().response.signature)].join('|'), \
     e => window.__passkey = 'error:' + e.name); true";

pub(super) fn check(h: &mut Harness) -> Result<(), Error> {
    let here: String = serde_json::from_str(&h.evaluate("location.href")?)?;
    h.command(superfik_core::BrowserOp::Navigate { url: here.replace("127.0.0.1", "localhost") })?;
    assert_eq!(h.evaluate("typeof window.__superfikPasskey")?, "\"undefined\"", "the page can reach the binding");
    h.evaluate(ASK)?;
    let ask = requested(h)?;
    let origin = h.evaluate("location.origin")?;
    assert_eq!(format!("\"{}\"", ask.origin), origin, "the origin comes from the page's context");
    assert_eq!((ask.challenge.as_str(), ask.allowed.as_slice()), ("AQID", &["BAUG".to_owned()][..]));
    let assertion = Assertion {
        credential_id: "BAUG".into(),
        client_data: "eyJ0eXBlIjoid2ViYXV0aG4uZ2V0In0".into(),
        authenticator_data: "AAAA".into(),
        signature: "MEUCIQ".into(),
        user_handle: None,
    };
    h.host.send(Call::AnswerPasskey { id: h.page, request: ask.request, reply: PasskeyReply::Signed(assertion) });
    let signed = settled(h)?;
    assert_eq!(
        signed, r#""true|BAUG|public-key|4|{\"type\":\"webauthn.get\"}|platform|\"MEUCIQ\"""#,
        "the page got a PublicKeyCredential"
    );
    h.evaluate(ASK)?;
    let ask = requested(h)?;
    h.host.send(Call::AnswerPasskey { id: h.page, request: ask.request, reply: PasskeyReply::Refused });
    assert_eq!(settled(h)?, "\"error:NotAllowedError\"");
    h.evaluate(&ASK.replace(
        "navigator.credentials.get({",
        "window.__abort = new AbortController(); navigator.credentials.get({ signal: window.__abort.signal,",
    ))?;
    let ask = requested(h)?;
    h.host.send(Call::AnswerPasskey { id: h.page, request: ask.request, reply: PasskeyReply::Elsewhere });
    h.evaluate("new Promise(r => setTimeout(r, 500))")?;
    assert_eq!(h.evaluate("window.__passkey")?, "\"waiting\"", "Chromium's own call is under way");
    h.evaluate("window.__abort.abort(); true")?;
    assert_eq!(settled(h)?, "\"error:AbortError\"", "the page's own call ran and was aborted");
    conditional(h)?;
    created(h)?;
    eprintln!(
        "Passkey acceptance passed: page origin, hidden binding, assertion, refusal, native fallback, conditional, \
         abort and registration"
    );
    Ok(())
}

fn conditional(h: &mut Harness) -> Result<(), Error> {
    h.evaluate(&ASK.replace(
        "navigator.credentials.get({",
        "window.__abort = new AbortController(); navigator.credentials.get({ mediation: 'conditional', signal: window.__abort.signal,",
    ))?;
    let ask = requested(h)?;
    assert!(ask.conditional, "the page offered it in a field");
    h.evaluate("window.__abort.abort(); true")?;
    assert_eq!(settled(h)?, "\"error:AbortError\"");
    h.host.send(Call::AnswerPasskey { id: h.page, request: ask.request, reply: PasskeyReply::Refused });
    h.evaluate("new Promise(r => setTimeout(r, 200))")?;
    assert_eq!(h.evaluate("window.__passkey")?, "\"error:AbortError\"", "a late answer is ignored");
    Ok(())
}

fn created(h: &mut Harness) -> Result<(), Error> {
    h.evaluate(
        "window.__passkey = 'waiting'; navigator.credentials.create({ publicKey: { rp: { name: 'Fixture' }, \
         user: { id: new Uint8Array([7]), name: 'fixture', displayName: 'Fixture' }, challenge: new Uint8Array([1, 2, 3]), \
         pubKeyCredParams: [{ type: 'public-key', alg: -7 }] } }).then(c => window.__passkey = [c instanceof PublicKeyCredential, \
         c.response instanceof AuthenticatorAttestationResponse, c.response.getPublicKeyAlgorithm(), \
         c.response.getTransports().join(','), c.response.attestationObject.byteLength].join('|'), \
         e => window.__passkey = 'error:' + e.name); true",
    )?;
    let ask = requested(h)?;
    let registration = ask.registration.ok_or("a create reached the host as a sign-in")?;
    assert_eq!(
        (registration.rp_name.as_str(), registration.user_name.as_str(), registration.user_id.as_str()),
        ("Fixture", "fixture", "Bw")
    );
    assert!(registration.es256);
    let attestation = Attestation {
        credential_id: "BAUG".into(),
        client_data: "eyJ0eXBlIjoid2ViYXV0aG4uY3JlYXRlIn0".into(),
        attestation_object: "o2NmbXRkbm9uZQ".into(),
        authenticator_data: "AAAA".into(),
        public_key: "MFkw".into(),
    };
    h.host.send(Call::AnswerPasskey { id: h.page, request: ask.request, reply: PasskeyReply::Created(attestation) });
    assert_eq!(settled(h)?, "\"true|true|-7|hybrid,internal|10\"", "the page got an attestation");
    Ok(())
}

fn requested(h: &mut Harness) -> Result<PasskeyAsk, Error> {
    if !h.passkeys.is_empty() {
        return Ok(h.passkeys.remove(0));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        h.event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut h.shell);
        for (_, event) in h.host.pump(&h.device, &h.queue) {
            if let PageEvent::PasskeyRequested(ask) = event {
                return Ok(ask);
            }
        }
    }
    Err("the page's passkey request never reached the host".into())
}

fn settled(h: &mut Harness) -> Result<String, Error> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let value = h.evaluate("window.__passkey")?;
        if value != "\"waiting\"" {
            return Ok(value);
        }
    }
    Err("the page's passkey promise never settled".into())
}
