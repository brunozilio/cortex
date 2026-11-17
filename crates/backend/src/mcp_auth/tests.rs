use std::sync::Arc;

use agent_engine::model::http::ScriptedHttp;
use domain::Principal;
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
    let owner = Principal::LocalOwner;
    let auth = McpAuth::new(Arc::clone(&secrets), http.clone()).at(1_000);
    let mut opened = String::new();
    auth.sign_in(&owner, SERVER, &mut |url| {
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
    assert_eq!(auth.token(&owner, SERVER).as_deref(), Some("a-1"));
    assert_eq!(auth.token(&owner, "https://other.test/mcp"), None, "only the server signed in to");

    let refreshing = Arc::new(ScriptedHttp::new([(200, json!({ "access_token": "a-2" }).to_string())]));
    let later = McpAuth::new(Arc::clone(&secrets), refreshing.clone()).at(1_000 + 3_600);
    assert_eq!(later.token(&owner, SERVER).as_deref(), Some("a-2"));
    assert!(refreshing.sent()[0].body.contains("grant_type=refresh_token&refresh_token=r-1"));

    assert_eq!(later.sign_out(&owner, SERVER), Ok(true));
    assert_eq!(later.token(&owner, SERVER), None);
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
    let error = auth.sign_in(&Principal::LocalOwner, SERVER, &mut |_| {}).expect_err("no registration");
    assert!(error.contains("register"), "{error}");
    let with_a_token = format!("{} mcp add -H 'Authorization: Bearer ${{TOKEN}}' <name> {SERVER}", brand::COMMAND);
    assert!(error.contains(&with_a_token), "says how to use a token instead: {error}");
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
    let error = auth.sign_in(&Principal::LocalOwner, SERVER, &mut |_| opened = true).expect_err("refused");
    assert!(error.contains("file:///Applications/Calculator.app") && !opened, "{error}");
    let plain = McpAuth::new(Arc::new(InMemorySecretStore::default()), Arc::new(ScriptedHttp::new([])));
    assert!(plain.sign_in(&Principal::LocalOwner, "http://mcp.test/mcp", &mut |_| {}).is_err(), "http elsewhere");
}

struct ReadOnlySecrets;

impl SecretStore for ReadOnlySecrets {
    fn get(&self, _: &Principal, _: domain::SecretId) -> Option<crate::secrets::Secrets> {
        None
    }

    fn set(&self, _: &Principal, _: domain::SecretId, _: &crate::secrets::Secrets) -> Result<(), String> {
        Err("Operation not permitted (os error 1)".into())
    }

    fn delete(&self, _: &Principal, _: domain::SecretId) {}
}

#[test]
fn a_sign_in_that_could_not_be_kept_stops_before_the_browser_opens() {
    let http = Arc::new(ScriptedHttp::new([]));
    let auth = McpAuth::new(Arc::new(ReadOnlySecrets), http.clone());
    let mut opened = false;
    let error = auth.sign_in(&Principal::LocalOwner, SERVER, &mut |_| opened = true).expect_err("cannot keep it");
    assert!(!opened, "the user is not sent to approve a sign-in that would be lost");
    assert!(error.contains("Operation not permitted") && error.contains("saved"), "{error}");
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
    let owner = Principal::LocalOwner;
    let servers: Vec<String> = (0..8).map(|n| format!("https://mcp{n}.test/mcp")).collect();
    let start = std::sync::Barrier::new(servers.len());
    std::thread::scope(|scope| {
        for server in &servers {
            let store = crate::secrets::SealedSecretStore::open(&file, [7; 32]).expect("store");
            let auth = McpAuth::new(Arc::new(store), Arc::new(ScriptedHttp::new([])));
            let (owner, start) = (owner.clone(), &start);
            scope.spawn(move || {
                start.wait();
                auth.save(&owner, server, Some(&tokens_for(server))).expect("saved");
            });
        }
    });
    let store = crate::secrets::SealedSecretStore::open(&file, [7; 32]).expect("store");
    let reader = McpAuth::new(Arc::new(store), Arc::new(ScriptedHttp::new([])));
    let kept: Vec<&String> = servers.iter().filter(|server| reader.token(&owner, server).is_some()).collect();
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
    auth.sign_in(&Principal::LocalOwner, SERVER, &mut |url| {
        opened = url.to_owned();
        approve(url);
    })
    .expect("signed in");
    assert!(opened.contains("&scope=mcp%3Aread%20mcp%3Awrite"), "{opened}");
    assert!(!opened.contains("admin"), "only what the resource asks for: {opened}");
}
