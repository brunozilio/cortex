use std::sync::Arc;

use agent_engine::model::http::ScriptedHttp;
use serde_json::json;

use super::*;
use crate::secrets::InMemorySecretStore;

const SERVER: &str = "https://mcp.test/mcp";

fn query_value(url: &str, key: &str) -> String {
    let query = url.split_once('?').map(|(_, query)| query).unwrap_or_default();
    let raw = query.split('&').find_map(|pair| pair.strip_prefix(&format!("{key}="))).unwrap_or_default();
    raw.replace("%3A", ":").replace("%2F", "/")
}

fn approve(url: &str) {
    let back = format!("{}?code=xyz&state={}", query_value(url, "redirect_uri"), query_value(url, "state"));
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder().no_proxy().build().expect("client");
        let _ = client.get(back).send();
    });
}

#[test]
fn signing_in_discovers_registers_and_keeps_tokens_that_are_refreshed_when_stale() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let http = Arc::new(ScriptedHttp::new([
        (200, json!({ "authorization_servers": ["https://auth.test"] }).to_string()),
        (
            200,
            json!({
                "authorization_endpoint": "https://auth.test/authorize",
                "token_endpoint": "https://auth.test/token",
                "registration_endpoint": "https://auth.test/register"
            })
            .to_string(),
        ),
        (201, json!({ "client_id": "c-1" }).to_string()),
        (200, json!({ "access_token": "a-1", "refresh_token": "r-1", "expires_in": 3600 }).to_string()),
    ]));
    let auth = McpAuth::new(Arc::clone(&secrets), http.clone()).at(1_000);
    let mut opened = String::new();
    auth.sign_in(SERVER, &Arc::default(), &mut |url| {
        opened = url.to_owned();
        approve(url);
    })
    .expect("signed in");

    assert!(opened.starts_with("https://auth.test/authorize?") && opened.contains("client_id=c-1"), "{opened}");
    let sent = http.sent();
    assert_eq!(sent[0].url, "https://mcp.test/.well-known/oauth-protected-resource/mcp");
    assert_eq!(sent[1].url, "https://auth.test/.well-known/oauth-authorization-server");
    let exchange = &sent[3].body;
    assert!(exchange.contains("grant_type=authorization_code") && exchange.contains("code=xyz"), "{exchange}");
    let verifier = exchange.split('&').find_map(|pair| pair.strip_prefix("code_verifier=")).expect("verifier");
    assert_eq!(
        query_value(&opened, "code_challenge"),
        llm_providers::oauth::challenge(verifier),
        "PKCE ties the two together"
    );
    assert_eq!(auth.token(SERVER).as_deref(), Some("a-1"));
    assert_eq!(auth.token("https://other.test/mcp"), None, "only the server signed in to");

    let refreshing = Arc::new(ScriptedHttp::new([(200, json!({ "access_token": "a-2" }).to_string())]));
    let later = McpAuth::new(Arc::clone(&secrets), refreshing.clone()).at(1_000 + 3_600);
    assert_eq!(later.token(SERVER).as_deref(), Some("a-2"));
    assert!(refreshing.sent()[0].body.contains("grant_type=refresh_token&refresh_token=r-1"));

    assert_eq!(later.sign_out(SERVER), Ok(true));
    assert_eq!(later.token(SERVER), None);
}

#[test]
fn a_server_that_does_not_let_brand_register_says_so() {
    let http = Arc::new(ScriptedHttp::new([
        (404, String::new()),
        (404, String::new()),
        (
            200,
            json!({ "authorization_endpoint": "https://mcp.test/authorize", "token_endpoint": "https://mcp.test/token" })
                .to_string(),
        ),
    ]));
    let auth = McpAuth::new(Arc::new(InMemorySecretStore::default()), http);
    let error = auth.sign_in(SERVER, &Arc::default(), &mut |_| {}).expect_err("no registration");
    assert_eq!(error, McpAuthError::NoRegistration { server: SERVER.to_owned() });
    let with_a_token = format!("{} mcp add -H 'Authorization: Bearer ${{TOKEN}}' <name> {SERVER}", brand::COMMAND);
    assert_eq!(
        error.to_string(),
        format!(
            "this server does not let applications register themselves, so {} cannot sign in. Use a token the \
             service issues instead, kept in the environment: `{with_a_token}`",
            brand::NAME
        ),
        "says how to use a token instead"
    );
}

#[test]
fn an_unsafe_sign_in_address_is_refused_before_the_browser_opens() {
    let metadata = json!({
        "authorization_endpoint": "file:///Applications/Calculator.app",
        "token_endpoint": "https://mcp.test/token",
        "registration_endpoint": "https://mcp.test/register"
    });
    let http = Arc::new(ScriptedHttp::new([(404, String::new()), (404, String::new()), (200, metadata.to_string())]));
    let auth = McpAuth::new(Arc::new(InMemorySecretStore::default()), http);
    let mut opened = false;
    let error = auth.sign_in(SERVER, &Arc::default(), &mut |_| opened = true).expect_err("refused");
    assert_eq!(error, McpAuthError::UnsafeAddress("file:///Applications/Calculator.app".into()));
    assert_eq!(
        error.to_string(),
        "file:///Applications/Calculator.app is not a safe address to sign in with (https, or http to this computer)"
    );
    assert!(!opened);
    let plain = McpAuth::new(Arc::new(InMemorySecretStore::default()), Arc::new(ScriptedHttp::new([])));
    assert!(plain.sign_in("http://mcp.test/mcp", &Arc::default(), &mut |_| {}).is_err(), "http elsewhere");
}

struct ReadOnlySecrets;

impl SecretStore for ReadOnlySecrets {
    fn get(&self, _: domain::SecretId) -> Option<crate::secrets::Secrets> {
        None
    }

    fn set(&self, _: domain::SecretId, _: &crate::secrets::Secrets) -> Result<(), String> {
        Err("Operation not permitted (os error 1)".into())
    }

    fn delete(&self, _: domain::SecretId) {}
}

#[test]
fn a_sign_in_that_could_not_be_kept_stops_before_the_browser_opens() {
    let http = Arc::new(ScriptedHttp::new([]));
    let auth = McpAuth::new(Arc::new(ReadOnlySecrets), http.clone());
    let mut opened = false;
    let error = auth.sign_in(SERVER, &Arc::default(), &mut |_| opened = true).expect_err("cannot keep it");
    assert!(!opened, "the user is not sent to approve a sign-in that would be lost");
    assert_eq!(
        error.to_string(),
        "a sign-in could not be saved here (Operation not permitted (os error 1)), so the browser was not opened"
    );
    assert!(matches!(error, McpAuthError::CannotKeep(_)), "{error:?}");
    assert!(http.sent().is_empty(), "nothing is asked of the server either");
}

fn tokens_for(server: &str) -> McpTokens {
    McpTokens {
        access_token: format!("a-{server}"),
        refresh_token: None,
        expires_at: None,
        client_id: "c".into(),
        client_secret: None,
        token_endpoint: "https://auth.test/token".into(),
        resource: server.to_owned(),
    }
}

#[test]
fn two_processes_signing_in_to_different_servers_keep_both_sign_ins() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("secrets.json");
    let servers: Vec<String> = (0..8).map(|n| format!("https://mcp{n}.test/mcp")).collect();
    let start = std::sync::Barrier::new(servers.len());
    std::thread::scope(|scope| {
        for server in &servers {
            let store = crate::secrets::SealedSecretStore::open(&file, [7; 32]).expect("store");
            let auth = McpAuth::new(Arc::new(store), Arc::new(ScriptedHttp::new([])));
            let start = &start;
            scope.spawn(move || {
                start.wait();
                auth.save(server, Some(&tokens_for(server))).expect("saved");
            });
        }
    });
    let store = crate::secrets::SealedSecretStore::open(&file, [7; 32]).expect("store");
    let reader = McpAuth::new(Arc::new(store), Arc::new(ScriptedHttp::new([])));
    let kept: Vec<&String> = servers.iter().filter(|server| reader.token(server).is_some()).collect();
    assert_eq!(kept.len(), servers.len(), "kept only {kept:?}");
}

#[test]
fn the_scopes_the_resource_asks_for_reach_the_authorization_url() {
    let http = Arc::new(ScriptedHttp::new([
        (
            200,
            json!({ "authorization_servers": ["https://auth.test"], "scopes_supported": ["mcp:read", "mcp:write"] })
                .to_string(),
        ),
        (
            200,
            json!({
                "authorization_endpoint": "https://auth.test/authorize",
                "token_endpoint": "https://auth.test/token",
                "registration_endpoint": "https://auth.test/register",
                "scopes_supported": ["mcp:read", "mcp:write", "admin"]
            })
            .to_string(),
        ),
        (201, json!({ "client_id": "c-1" }).to_string()),
        (200, json!({ "access_token": "a-1" }).to_string()),
    ]));
    let auth = McpAuth::new(Arc::new(InMemorySecretStore::default()), http);
    let mut opened = String::new();
    auth.sign_in(SERVER, &Arc::default(), &mut |url| {
        opened = url.to_owned();
        approve(url);
    })
    .expect("signed in");
    assert!(opened.contains("&scope=mcp%3Aread%20mcp%3Awrite"), "{opened}");
    assert!(!opened.contains("admin"), "only what the resource asks for: {opened}");
}

#[test]
fn a_cancelled_sign_in_stops_waiting_for_the_browser_and_frees_its_port() {
    let http = Arc::new(ScriptedHttp::new([
        (200, json!({ "authorization_servers": ["https://auth.test"] }).to_string()),
        (
            200,
            json!({
                "authorization_endpoint": "https://auth.test/authorize",
                "token_endpoint": "https://auth.test/token",
                "registration_endpoint": "https://auth.test/register"
            })
            .to_string(),
        ),
        (201, json!({ "client_id": "c-1" }).to_string()),
    ]));
    let auth = McpAuth::new(Arc::new(InMemorySecretStore::default()), http);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut redirect = String::new();
    let asked = std::time::Instant::now();
    let error = auth
        .sign_in(SERVER, &stop, &mut |url| {
            redirect = query_value(url, "redirect_uri");
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
        })
        .expect_err("cancelled");
    assert!(error.to_string().contains("cancelled"), "{error}");
    assert!(asked.elapsed() < std::time::Duration::from_secs(5), "it did not wait for the browser");
    let port: u16 = redirect.rsplit(':').next().and_then(|rest| rest.split('/').next()?.parse().ok()).expect("port");
    let freed = std::time::Instant::now();
    while let Err(error) = Loopback::bind_on(port, "/callback", false) {
        assert!(freed.elapsed() < std::time::Duration::from_secs(5), "the port is still held: {error:?}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[derive(Debug)]
struct SingleUseRefreshTokens {
    valid: Mutex<String>,
    issued: std::sync::atomic::AtomicUsize,
    refused: std::sync::atomic::AtomicUsize,
}

impl Http for SingleUseRefreshTokens {
    fn post(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        body: &str,
    ) -> Result<agent_engine::model::http::Response, String> {
        use std::sync::atomic::Ordering;
        let presented =
            body.split('&').find_map(|pair| pair.strip_prefix("refresh_token=")).unwrap_or_default().to_owned();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let mut valid = self.valid.lock().expect("valid token");
        let (status, answer) = if presented == *valid {
            let next = self.issued.fetch_add(1, Ordering::SeqCst) + 2;
            *valid = format!("r-{next}");
            (200, json!({ "access_token": format!("a-{next}"), "refresh_token": *valid, "expires_in": 3600 }))
        } else {
            self.refused.fetch_add(1, Ordering::SeqCst);
            (400, json!({ "error": "invalid_grant" }))
        };
        Ok(agent_engine::model::http::Response {
            status,
            headers: Vec::new(),
            body: agent_engine::model::http::Body::from(answer.to_string()),
        })
    }

    fn get(&self, _url: &str, _headers: &[(String, String)]) -> Result<agent_engine::model::http::Response, String> {
        Err("not used".to_owned())
    }
}

#[test]
fn two_processes_with_a_stale_mcp_sign_in_refresh_it_once_and_both_get_a_token() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = tempfile::tempdir().expect("dir");
    let stale = McpTokens {
        access_token: "a-1".to_owned(),
        refresh_token: Some("r-1".to_owned()),
        expires_at: Some(1_000),
        client_id: "c-1".to_owned(),
        client_secret: None,
        token_endpoint: "https://auth.test/token".to_owned(),
        resource: SERVER.to_owned(),
    };
    let endpoint = Arc::new(SingleUseRefreshTokens {
        valid: Mutex::new("r-1".to_owned()),
        issued: AtomicUsize::new(0),
        refused: AtomicUsize::new(0),
    });
    McpAuth::new(crate::secrets::secrets_in(Some(dir.path())), endpoint.clone())
        .save(SERVER, Some(&stale))
        .expect("signed in");
    let start = Arc::new(std::sync::Barrier::new(2));

    let processes: Vec<_> = (0..2)
        .map(|_| {
            let auth = McpAuth::new(crate::secrets::secrets_in(Some(dir.path())), endpoint.clone()).at(5_000);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                auth.token(SERVER)
            })
        })
        .collect();
    let tokens: Vec<_> = processes.into_iter().map(|process| process.join().expect("process")).collect();

    assert_eq!(tokens, [Some("a-2".to_owned()), Some("a-2".to_owned())], "neither process lost the sign-in");
    assert_eq!(endpoint.refused.load(Ordering::SeqCst), 0, "no refresh token was presented twice");
    assert_eq!(endpoint.issued.load(Ordering::SeqCst), 1, "the second process used the first one's new token");
    let kept = McpAuth::new(crate::secrets::secrets_in(Some(dir.path())), endpoint.clone()).load(SERVER);
    assert_eq!(kept.and_then(|tokens| tokens.refresh_token).as_deref(), Some("r-2"));
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("refresh"))
        .collect();
    assert!(leftovers.is_empty(), "the refresh lock leaves nothing behind: {leftovers:?}");
}

#[test]
fn each_server_refreshes_under_its_own_lock() {
    assert_ne!(refresh_slot(SERVER), refresh_slot("https://other.test/mcp"));
    assert_eq!(refresh_slot(SERVER), refresh_slot(SERVER));
}
