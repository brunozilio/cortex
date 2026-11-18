use std::sync::Arc;

use agent_engine::model::http::ScriptedHttp;
use serde_json::json;

use super::*;
use crate::secrets::InMemorySecretStore;

fn query_value(url: &str, key: &str) -> String {
    let query = url.split_once('?').map(|(_, query)| query).unwrap_or_default();
    let raw = query.split('&').find_map(|pair| pair.strip_prefix(&format!("{key}="))).unwrap_or_default();
    raw.replace("%3A", ":").replace("%2F", "/").replace("%20", " ").replace('+', " ")
}

fn approve(url: &str) {
    let back = format!("{}?code=xyz&state={}", query_value(url, "redirect_uri"), query_value(url, "state"));
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder().no_proxy().build().expect("client");
        let _ = client.get(back).send();
    });
}

fn client(id: &str) -> CloudflareClient {
    CloudflareClient::parse(id, "http://127.0.0.1:0/oauth/callback", "account:read offline_access").expect("client")
}

fn signed_in(secrets: &Arc<dyn SecretStore>) -> (Arc<ScriptedHttp>, String) {
    let http = Arc::new(ScriptedHttp::new([(
        200,
        json!({ "access_token": "a-1", "refresh_token": "r-1", "expires_in": 3600 }).to_string(),
    )]));
    let sign_in = CloudflareSignIn::new(Arc::clone(secrets), http.clone(), client("wrangler")).at(1_000);
    let mut opened = String::new();
    let token = sign_in
        .token(&Arc::default(), &mut |url| {
            opened = url.to_owned();
            approve(url);
        })
        .expect("signed in");
    assert_eq!(token, "a-1");
    (http, opened)
}

#[test]
fn the_browser_sign_in_uses_pkce_with_the_configured_client_and_scopes() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let (http, opened) = signed_in(&secrets);

    assert!(opened.starts_with("https://dash.cloudflare.com/oauth2/auth?"), "{opened}");
    assert_eq!(query_value(&opened, "client_id"), "wrangler");
    assert_eq!(query_value(&opened, "scope"), "account:read offline_access");
    assert_eq!(query_value(&opened, "code_challenge_method"), "S256");
    let sent = http.sent();
    assert_eq!(sent[0].url, TOKEN);
    let exchange = &sent[0].body;
    assert!(exchange.contains("grant_type=authorization_code") && exchange.contains("code=xyz"), "{exchange}");
    let verifier = exchange.split('&').find_map(|pair| pair.strip_prefix("code_verifier=")).expect("verifier");
    assert_eq!(query_value(&opened, "code_challenge"), challenge(verifier));
}

#[test]
fn a_kept_sign_in_is_reused_then_refreshed_when_stale_and_forgotten_on_sign_out() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    signed_in(&secrets);

    let quiet = Arc::new(ScriptedHttp::new([]));
    let soon = CloudflareSignIn::new(Arc::clone(&secrets), quiet.clone(), client("wrangler")).at(1_100);
    assert_eq!(soon.token(&Arc::default(), &mut |_| panic!("no browser")).as_deref(), Ok("a-1"));
    assert!(quiet.sent().is_empty());

    let refreshing = Arc::new(ScriptedHttp::new([
        (200, json!({ "access_token": "a-2", "expires_in": 3600 }).to_string()),
        (200, "{}".to_owned()),
    ]));
    let later = CloudflareSignIn::new(Arc::clone(&secrets), refreshing.clone(), client("wrangler")).at(1_000 + 3_600);
    assert_eq!(later.kept().as_deref(), Some("a-2"));
    let refresh = &refreshing.sent()[0].body;
    assert!(refresh.contains("grant_type=refresh_token&refresh_token=r-1&client_id=wrangler"), "{refresh}");

    assert_eq!(later.sign_out(), Ok(true));
    assert_eq!(refreshing.sent()[1].url, REVOKE);
    assert_eq!(later.kept(), None);
    assert_eq!(later.sign_out(), Ok(false));
}

#[test]
fn a_sign_in_made_with_another_client_is_not_used() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    signed_in(&secrets);
    let production = CloudflareSignIn::new(secrets, Arc::new(ScriptedHttp::new([])), client("ours")).at(1_100);
    assert_eq!(production.kept(), None);
}

#[test]
fn the_redirect_is_a_fixed_loopback_address() {
    let parsed = CloudflareClient::parse("id", "http://localhost:8976/oauth/callback", "s").expect("parsed");
    assert_eq!((parsed.port, parsed.path.as_str(), parsed.localhost), (8976, "/oauth/callback", true));
    for bad in ["https://localhost:8976/cb", "http://example.com:80/cb", "http://localhost/cb", "http://localhost:x/cb"]
    {
        assert!(CloudflareClient::parse("id", bad, "s").is_err(), "{bad}");
    }
    assert!(CloudflareClient::branded().is_ok(), "config.toml names a usable client");
}

#[derive(Debug)]
struct SingleUseRefreshTokens {
    valid: std::sync::Mutex<String>,
    issued: std::sync::atomic::AtomicUsize,
    refused: std::sync::atomic::AtomicUsize,
}

impl agent_engine::model::http::Http for SingleUseRefreshTokens {
    fn post(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        body: &str,
    ) -> Result<agent_engine::model::http::Response, String> {
        use std::sync::atomic::Ordering;
        let presented = body.split('&').find_map(|pair| pair.strip_prefix("refresh_token=")).unwrap_or_default();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut valid = self.valid.lock().expect("valid");
        let (status, answer) = if presented == *valid {
            let next = self.issued.fetch_add(1, Ordering::SeqCst) + 2;
            *valid = format!("r-{next}");
            (200, json!({ "access_token": format!("a-{next}"), "refresh_token": *valid, "expires_in": 3600 }))
        } else {
            self.refused.fetch_add(1, Ordering::SeqCst);
            (400, json!({ "error": "invalid_grant" }))
        };
        let body = agent_engine::model::http::Body::from(answer.to_string());
        Ok(agent_engine::model::http::Response { status, headers: Vec::new(), body })
    }
}

#[test]
fn two_processes_with_a_stale_cloudflare_sign_in_refresh_it_once_and_both_keep_it() {
    use std::sync::atomic::Ordering;
    let dir = tempfile::tempdir().expect("dir");
    let stale = McpTokens {
        access_token: "a-1".into(),
        refresh_token: Some("r-1".into()),
        expires_at: Some(10),
        client_id: "wrangler".into(),
        client_secret: None,
        token_endpoint: TOKEN.into(),
        resource: String::new(),
    };
    let first = crate::secrets::secrets_in(Some(dir.path()));
    CloudflareSignIn::new(first, Arc::new(ScriptedHttp::new([])), client("wrangler")).save(Some(&stale)).expect("kept");
    let endpoint = Arc::new(SingleUseRefreshTokens {
        valid: std::sync::Mutex::new("r-1".into()),
        issued: 0.into(),
        refused: 0.into(),
    });
    let start = Arc::new(std::sync::Barrier::new(2));
    let processes: Vec<_> = (0..2)
        .map(|_| {
            let (endpoint, start) = (Arc::clone(&endpoint), Arc::clone(&start));
            let vault = crate::secrets::secrets_in(Some(dir.path()));
            std::thread::spawn(move || {
                let sign_in = CloudflareSignIn::new(vault, endpoint, client("wrangler")).at(1_000);
                start.wait();
                sign_in.kept()
            })
        })
        .collect();
    let tokens: Vec<_> = processes.into_iter().map(|process| process.join().expect("process")).collect();
    assert_eq!(tokens, [Some("a-2".to_owned()), Some("a-2".to_owned())], "both stay signed in");
    assert_eq!(endpoint.refused.load(Ordering::SeqCst), 0, "no refresh token was presented twice");
    assert_eq!(endpoint.issued.load(Ordering::SeqCst), 1);
}
