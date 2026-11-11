use std::sync::mpsc::channel;

use agent_engine::model::http::ScriptedHttp;

use super::*;

const ARN: &str = "arn:aws:codewhisperer:us-east-1:111122223333:profile/ABCDEF";

fn random(length: usize) -> Result<String, String> {
    Ok(format!("r{length}x{}", std::time::SystemTime::now().elapsed().map(|d| d.as_nanos()).unwrap_or(0)))
}

fn get(url: &str) -> (u16, Option<String>) {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client");
    let response = client.get(url).send().expect("get");
    let location = response.headers().get("location").and_then(|value| value.to_str().ok()).map(str::to_owned);
    (response.status().as_u16(), location)
}

fn param(url: &str, name: &str) -> String {
    let url = reqwest::Url::parse(url).expect("url");
    url.query_pairs().find(|(key, _)| key == name).map(|(_, value)| value.into_owned()).unwrap_or_default()
}

#[test]
fn an_organization_sign_in_goes_on_to_its_identity_center_in_the_same_tab() {
    let listener = Loopback::bind_on(0, "/oauth/callback", true).expect("bind");
    let port = listener.port();
    let (portal_tx, portal_rx) = channel::<String>();
    let browser = std::thread::spawn(move || {
        let portal = portal_rx.recv().expect("portal");
        assert!(portal.starts_with("https://app.kiro.dev/signin?"), "{portal}");
        let state = param(&portal, "state");
        let (status, next) = get(&format!(
            "http://127.0.0.1:{port}/signin/callback?login_option=awsidc\
             &issuer_url=https%3A%2F%2Facme.awsapps.com%2Fstart&idc_region=eu-west-1&state={state}"
        ));
        assert_eq!(status, 302);
        let next = next.expect("sent on");
        assert!(next.starts_with("https://oidc.eu-west-1.amazonaws.com/authorize?"), "{next}");
        assert_eq!(param(&next, "client_id"), "cid");
        assert_eq!(param(&next, "redirect_uri"), format!("http://127.0.0.1:{port}/oauth/callback"));
        let state = param(&next, "state");
        get(&format!("http://127.0.0.1:{port}/oauth/callback?code=org-code&state={state}")).0
    });
    let http = ScriptedHttp::new([
        (200, r#"{"clientId":"cid","clientSecret":"secret"}"#.to_owned()),
        (200, r#"{"accessToken":"a","refreshToken":"r","expiresIn":3600}"#.to_owned()),
        (200, format!(r#"{{"profiles":[{{"arn":"{ARN}"}}]}}"#)),
    ]);
    let mut open = |url: String, _: Option<String>| portal_tx.send(url).expect("send");
    let tokens = sign_in(&http, &listener, &mut open, &random).expect("signed in");
    assert_eq!(browser.join().expect("browser"), 200, "the tab says it is done");
    assert_eq!((tokens.access_token.as_str(), tokens.account_id.as_str()), ("a", ARN));
    assert!(tokens.idc.is_some(), "kept for refreshes");
    let exchange = &http.sent()[1];
    assert!(exchange.body.contains("\"code\":\"org-code\""), "{}", exchange.body);
}

#[test]
fn an_organization_answer_without_its_start_url_fails_in_the_tab_and_the_terminal() {
    let listener = Loopback::bind_on(0, "/oauth/callback", true).expect("bind");
    let port = listener.port();
    let (portal_tx, portal_rx) = channel::<String>();
    let browser = std::thread::spawn(move || {
        let state = param(&portal_rx.recv().expect("portal"), "state");
        get(&format!(
            "http://127.0.0.1:{port}/signin/callback?login_option=awsidc&issuer_url=&amplitude_device_id=&state={state}"
        ))
        .0
    });
    let http = ScriptedHttp::new([]);
    let mut open = |url: String, _: Option<String>| portal_tx.send(url).expect("send");
    let error = sign_in(&http, &listener, &mut open, &random).expect_err("no start URL");
    assert!(error.contains("issuer_url"), "{error}");
    assert_eq!(browser.join().expect("browser"), 200, "the tab is answered, not a bare 404");
    assert!(http.sent().is_empty());
}
