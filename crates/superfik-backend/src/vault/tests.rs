use std::sync::mpsc;

use serde_json::json;
use superfik_core::autofill::{LoginUri, UriRule};
use superfik_core::{AskKind, EngineEvent, VaultKind};

use super::*;

#[derive(Default)]
struct FakeBitwarden {
    calls: Mutex<Vec<String>>,
    inputs: Mutex<Vec<(String, String)>>,
}

impl BitwardenCli for FakeBitwarden {
    fn run_with_input(&self, args: &[&str], env: &[(&str, &str)], input: &str) -> Result<String, String> {
        if !env.iter().any(|(k, v)| *k == "BW_SESSION" && *v == "session-1") {
            return Err("Vault is locked.".into());
        }
        self.inputs.lock().expect("inputs").push((args.join(" "), input.to_owned()));
        Ok("{}".into())
    }

    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
        self.calls.lock().expect("calls").push(args.join(" "));
        let has = |key: &str, value: &str| env.iter().any(|(k, v)| *k == key && *v == value);
        match args {
            ["status"] => Ok(r#"{"status":"locked"}"#.into()),
            ["unlock", ..] if has("BW_PASSWORD", "hunter2") => Ok("session-1\n".into()),
            ["unlock", ..] => Err("Invalid master password.".into()),
            _ if !has("BW_SESSION", "session-1") => Err("Vault is locked.".into()),
            ["sync"] => Ok(String::new()),
            ["list", "folders", ..] => {
                Ok(r#"[{"id":"f1","name":"Superfik"},{"id":"f2","name":"Superfik old"}]"#.into())
            }
            ["list", "items", "--folderid", "f1"] => Ok(json!([{
                "name": "GitHub",
                "login": {
                    "username": "bruno", "password": "ghp_secret_token",
                    "uris": [{ "uri": "https://github.com", "match": null }]
                },
                "fields": [{ "name": "otp", "value": null }]
            }])
            .to_string()),
            ["list", "items", "--url", _] => Ok(json!([{
                "name": "Google",
                "login": {
                    "username": "ana@example.com", "password": "google-password",
                    "uris": [{ "uri": "https://accounts.google.com", "match": null }]
                }
            }])
            .to_string()),
            _ => Err(format!("unexpected: {}", args.join(" "))),
        }
    }
}

struct EchoRunner;

impl CommandRunner for EchoRunner {
    fn run(&self, command: &str, cwd: &std::path::Path, env: &[(String, String)], _: Duration) -> Result<Ran, String> {
        let vars: Vec<String> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
        Ok(Ran {
            code: Some(0),
            output: format!("{command} in {} {}", cwd.display(), vars.join(" ")),
            timed_out: false,
        })
    }
}

fn account(password: &str) -> VaultAccount {
    VaultAccount {
        title: "Bitwarden · Superfik".into(),
        kind: VaultKind::Bitwarden,
        scope: "superfik".into(),
        secrets: crate::plugins::Secrets::from([("password".to_owned(), password.to_owned())]),
    }
}

fn rig() -> (Vault, mpsc::Receiver<(RunId, EngineEvent)>, Arc<FakeBitwarden>) {
    let (sent, asks) = mpsc::channel();
    let sent = Mutex::new(sent);
    let notify: Notify = Arc::new(move |run, event| {
        let _ = sent.lock().expect("sender").send((run, event));
    });
    let bitwarden = Arc::new(FakeBitwarden::default());
    let providers = VaultProviders::default().with(VaultKind::Bitwarden, Arc::new(Bitwarden::new(bitwarden.clone())));
    (Vault::new(providers, Arc::new(EchoRunner), notify), asks, bitwarden)
}

fn call(vault: &Vault, token: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
    vault.handle(token, &message).expect("token").expect("answer")["result"].clone()
}

fn run_args() -> Value {
    json!({ "command": "gh auth status", "secrets": [{ "item": "GitHub", "field": "password", "env": "GH_TOKEN" }] })
}

#[test]
fn listing_names_items_without_values_and_unlocks_once() {
    let (vault, _, bitwarden) = rig();
    let tool = vault.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![account("hunter2")]);
    let listed = call(&vault, &tool.token, LIST_SECRETS_TOOL, json!({}));
    assert_eq!(listed["content"][0]["text"], "Bitwarden · Superfik (folder superfik):\n- GitHub: password, username");
    call(&vault, &tool.token, LIST_SECRETS_TOOL, json!({}));
    let calls = bitwarden.calls.lock().expect("calls");
    assert_eq!(calls.iter().filter(|c| c.starts_with("unlock")).count(), 1);
    assert!(
        calls.iter().all(|c| !c.contains("hunter2") && !c.contains("session-1")),
        "secrets stay off the command line"
    );
}

#[test]
fn an_allowed_run_gets_the_secret_and_the_agent_never_sees_it() {
    let (vault, asks, _) = rig();
    let run = RunId::new();
    let tool = vault.grant(&Principal::LocalOwner, run, "/repo".into(), vec![account("hunter2")]);
    let answering = vault.clone();
    let user = std::thread::spawn(move || {
        let (asked_run, EngineEvent::Asked(ask)) = asks.recv().expect("ask") else { panic!("not an ask") };
        assert_eq!(asked_run, run);
        let AskKind::Permission { tool, subject, rememberable } = &ask.kind else { panic!("permission") };
        assert_eq!((tool.as_str(), rememberable), ("Bitwarden", &false));
        assert_eq!(subject, "GitHub · password → $GH_TOKEN\n$ gh auth status");
        let stranger = Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new());
        assert!(answering.answer(&stranger, &ask.id, &Decision::Deny), "claimed, but only the owner decides");
        assert!(answering.answer(&Principal::LocalOwner, &ask.id, &Decision::Allow));
    });
    let result = call(&vault, &tool.token, RUN_WITH_SECRETS_TOOL, run_args());
    user.join().expect("user");
    assert_eq!(result["isError"], Value::Null);
    assert_eq!(result["content"][0]["text"], "exit 0\ngh auth status in /repo GH_TOKEN=[secret $GH_TOKEN]");
}

#[test]
fn a_denied_or_ended_run_runs_nothing() {
    let (vault, asks, _) = rig();
    let tool = vault.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![account("hunter2")]);
    let answering = vault.clone();
    let user = std::thread::spawn(move || {
        let (_, EngineEvent::Asked(ask)) = asks.recv().expect("ask") else { panic!("not an ask") };
        answering.answer(&Principal::LocalOwner, &ask.id, &Decision::Deny);
        let (_, EngineEvent::Asked(_)) = asks.recv().expect("second ask") else { panic!("not an ask") };
        answering.revoke(&tool_token(&answering));
    });
    let denied = call(&vault, &tool.token, RUN_WITH_SECRETS_TOOL, run_args());
    assert_eq!(denied["isError"], true);
    assert!(denied["content"][0]["text"].as_str().is_some_and(|t| t.contains("did not allow")));
    let ended = call(&vault, &tool.token, RUN_WITH_SECRETS_TOOL, run_args());
    user.join().expect("user");
    assert_eq!(ended["isError"], true, "the run ended while asking");
    assert_eq!(vault.handle(&tool.token, &json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })), Err(401));
}

fn tool_token(vault: &Vault) -> String {
    vault.lock().keys().next().cloned().expect("a grant")
}

#[test]
fn bad_names_and_passwords_fail_before_asking() {
    let (vault, asks, _) = rig();
    let tool = vault.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![account("hunter2")]);
    let unknown = json!({ "command": "x", "secrets": [{ "item": "Slack", "field": "password", "env": "T" }] });
    assert_eq!(call(&vault, &tool.token, RUN_WITH_SECRETS_TOOL, unknown)["isError"], true);
    let wrong = vault.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![account("nope")]);
    let locked = call(&vault, &wrong.token, LIST_SECRETS_TOOL, json!({}));
    assert_eq!(
        locked["content"][0]["text"],
        "Bitwarden · Superfik: could not unlock Bitwarden: Invalid master password."
    );
    assert!(asks.try_recv().is_err(), "nothing was asked");
    assert!(!vault.answer(&Principal::LocalOwner, &AskId("r1".into()), &Decision::Allow), "engine asks pass through");
}

#[test]
fn long_output_is_clipped() {
    let text = "a".repeat(MAX_OUTPUT + 5);
    assert!(clip(&text).ends_with("… (5 more characters)"));
    assert_eq!(clip("short"), "short");
}

#[test]
fn items_keep_the_addresses_they_were_saved_for() {
    let items = bitwarden::parse_items(
        &json!([
            { "name": "GitHub", "login": { "username": "bruno", "password": "p", "uris": [
                { "uri": "https://github.com/login", "match": null },
                { "uri": "https://gist.github.com", "match": 1 },
                { "uri": "^https://ghe\\.", "match": 4 },
                { "uri": "", "match": 0 }
            ] } },
            { "name": "Note", "notes": "n" }
        ])
        .to_string(),
    )
    .expect("parses");
    assert_eq!(
        items[0].uris,
        [
            LoginUri::new("https://github.com/login", None),
            LoginUri::new("https://gist.github.com", Some(UriRule::Host)),
            LoginUri::new("^https://ghe\\.", Some(UriRule::RegularExpression)),
        ]
    );
    assert!(items[1].uris.is_empty());
    assert!(!format!("{:?}", items[0]).contains("bruno"), "debug shows no values");
}

#[test]
fn a_sign_in_asks_the_owner_and_opens_only_the_runs_accounts() {
    let (vault, asks, _) = rig();
    let run = RunId::new();
    vault.grant(&Principal::LocalOwner, run, "/repo".into(), vec![account("hunter2")]);
    let answering = vault.clone();
    let user = std::thread::spawn(move || {
        let (_, EngineEvent::Asked(ask)) = asks.recv().expect("ask") else { panic!("not an ask") };
        let AskKind::Permission { subject, .. } = &ask.kind else { panic!("permission") };
        assert_eq!(subject, "GitHub → sign in on github.com");
        answering.answer(&Principal::LocalOwner, &ask.id, &Decision::Allow);
        let (_, EngineEvent::Asked(ask)) = asks.recv().expect("second ask") else { panic!("not an ask") };
        answering.answer(&Principal::LocalOwner, &ask.id, &Decision::Deny);
    });
    let subject = || "GitHub → sign in on github.com".to_owned();
    let allowed = vault.approve(&Principal::LocalOwner, run, "GitHub", subject()).expect("allowed");
    assert_eq!(allowed.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["Bitwarden · Superfik"]);
    let denied = vault.approve(&Principal::LocalOwner, run, "GitHub", subject());
    assert!(denied.is_err_and(|e| e.contains("did not allow")));
    user.join().expect("user");
    let stranger = Principal::member(superfik_core::UserId::new(), superfik_core::OrgId::new());
    assert!(vault.approve(&stranger, run, "GitHub", subject()).is_err_and(|e| e.contains("no vault account")));
    assert!(vault.approve(&Principal::LocalOwner, RunId::new(), "GitHub", subject()).is_err());
}

#[test]
fn a_new_login_goes_into_the_folder_on_standard_input_only() {
    use base64::Engine;
    let cli = FakeBitwarden::default();
    let login = superfik_core::autofill::NewLogin {
        name: "github.com".into(),
        url: "https://github.com/login".into(),
        username: "bruno".into(),
        password: "hunter2-pw".into(),
    };
    bitwarden::create_login(&cli, "session-1", "Superfik", &login).expect("created");
    assert!(!cli.calls.lock().expect("calls").iter().any(|c| c.contains("hunter2-pw")), "never on the command line");
    bitwarden::create_login(&cli, "session-1", "Nowhere", &login).expect("created outside any folder");
    let inputs = cli.inputs.lock().expect("inputs");
    let decode = |input: &str| -> Value {
        serde_json::from_slice(&base64::engine::general_purpose::STANDARD.decode(input).expect("base64")).expect("json")
    };
    let [(command, first), (_, second)] = &inputs[..] else { panic!("two items created: {inputs:?}") };
    assert_eq!(command, "create item");
    let item = decode(first);
    assert_eq!(item["folderId"], "f1");
    assert_eq!(item["login"]["password"], "hunter2-pw");
    assert_eq!(item["login"]["uris"][0]["uri"], "https://github.com/login");
    assert_eq!(decode(second)["folderId"], Value::Null, "no folder of that name");
}

#[test]
fn the_browser_finds_logins_anywhere_in_the_vault_not_only_in_the_folder() {
    let bitwarden = Arc::new(FakeBitwarden::default());
    let providers = VaultProviders::default().with(VaultKind::Bitwarden, Arc::new(Bitwarden::new(bitwarden)));
    let logins = Logins::new(providers);
    let account = VaultAccount { scope: "Pessoal".into(), title: "Bitwarden · Pessoal".into(), ..account("hunter2") };
    let url = "https://accounts.google.com/v3/signin/identifier";
    let found = logins.candidates(&Principal::LocalOwner, std::slice::from_ref(&account), url).expect("found");
    assert_eq!(found.iter().map(|c| c.item.as_str()).collect::<Vec<_>>(), ["Google"]);
    let fill =
        logins.fill(&Principal::LocalOwner, &[account], url, "Bitwarden · Pessoal", "Google", false).expect("fills");
    assert_eq!(fill.username.as_deref(), Some("ana@example.com"));
}

mod credentials;
