//! Bitwarden through its CLI (`bw`), already signed in on this machine.
//! The master password unlocks a session key kept for the run; items come
//! from one folder only. [`BitwardenCli`] is the IO; [`Bitwarden`] is the
//! provider.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use superfik_core::VaultKind;
use superfik_core::autofill::{LoginUri, NewLogin, UriRule};
use superfik_workspace::executable;

use super::bitwarden_passkeys::{keep_passkey, parse_passkeys};
use super::provider::{Item, KeptPasskey, VaultAccount, VaultPasskey, VaultProvider};

pub trait BitwardenCli: Send + Sync {
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Result<String, String>;

    fn run_with_input(&self, args: &[&str], env: &[(&str, &str)], input: &str) -> Result<String, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemBitwarden;

impl BitwardenCli for SystemBitwarden {
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
        bw(args, env, None)
    }

    fn run_with_input(&self, args: &[&str], env: &[(&str, &str)], input: &str) -> Result<String, String> {
        bw(args, env, Some(input))
    }
}

fn bw(args: &[&str], env: &[(&str, &str)], input: Option<&str>) -> Result<String, String> {
    use std::io::Write;
    use std::process::Stdio;
    let path = executable::locate("bw", "SUPERFIK_BW")
        .ok_or("the Bitwarden CLI (`bw`) is not installed: `npm install -g @bitwarden/cli`")?;
    let mut child = superfik_process::command(path)
        .args(args)
        .envs(env.iter().copied())
        .env("BW_NOINTERACTION", "true")
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input.as_bytes()).map_err(|error| error.to_string())?;
    }
    let output = child.wait_with_output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

pub struct Bitwarden {
    cli: Arc<dyn BitwardenCli>,
}

impl Bitwarden {
    pub fn new(cli: Arc<dyn BitwardenCli>) -> Self {
        Self { cli }
    }
}

impl std::fmt::Debug for Bitwarden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bitwarden").finish_non_exhaustive()
    }
}

impl VaultProvider for Bitwarden {
    fn items(&self, account: &VaultAccount, session: &mut Option<String>) -> Result<Vec<Item>, String> {
        let key = self.session(account, session)?;
        folder_items(self.cli.as_ref(), &key, &account.scope)
    }

    fn logins(&self, account: &VaultAccount, session: &mut Option<String>, url: &str) -> Result<Vec<Item>, String> {
        let key = self.session(account, session)?;
        parse_items(&self.cli.run(&["list", "items", "--url", url], &[("BW_SESSION", &key)])?)
    }

    fn refresh(&self, _account: &VaultAccount, session: &str) -> Result<(), String> {
        self.cli.run(&["sync"], &[("BW_SESSION", session)]).map(|_| ())
    }

    fn save(&self, account: &VaultAccount, session: &mut Option<String>, login: &NewLogin) -> Result<(), String> {
        let key = self.session(account, session)?;
        create_login(self.cli.as_ref(), &key, &account.scope, login)
    }

    fn passkeys(&self, account: &VaultAccount, session: &mut Option<String>) -> Result<Vec<VaultPasskey>, String> {
        let key = self.session(account, session)?;
        parse_passkeys(&self.cli.run(&["list", "items"], &[("BW_SESSION", &key)])?)
    }

    fn keep_passkey(
        &self,
        account: &VaultAccount,
        session: &mut Option<String>,
        new: &KeptPasskey,
    ) -> Result<(), String> {
        let key = self.session(account, session)?;
        keep_passkey(self.cli.as_ref(), &key, &account.scope, new)
    }
}

impl Bitwarden {
    fn session(&self, account: &VaultAccount, session: &mut Option<String>) -> Result<String, String> {
        if let Some(key) = session {
            return Ok(key.clone());
        }
        let password = account.secrets.get(VaultKind::Bitwarden.unlock_secret()).ok_or("no master password")?;
        Ok(session.insert(unlock(self.cli.as_ref(), password)?).clone())
    }
}

pub fn create_login(cli: &dyn BitwardenCli, session: &str, folder: &str, login: &NewLogin) -> Result<(), String> {
    use base64::Engine;
    let env = [("BW_SESSION", session)];
    let folders = cli.run(&["list", "folders", "--search", folder], &env)?;
    let id = folder_id(&folders, folder)?;
    let item = serde_json::json!({
        "type": 1,
        "name": login.name,
        "folderId": id,
        "organizationId": null,
        "notes": null,
        "favorite": false,
        "reprompt": 0,
        "fields": [],
        "login": {
            "username": login.username,
            "password": login.password,
            "uris": [{ "uri": login.url, "match": null }],
        },
    });
    let encoded = base64::engine::general_purpose::STANDARD.encode(item.to_string());
    cli.run_with_input(&["create", "item"], &env, &encoded).map(|_| ())
}

pub fn unlock(cli: &dyn BitwardenCli, password: &str) -> Result<String, String> {
    let status = cli.run(&["status"], &[])?;
    let status: Value = serde_json::from_str(&status).map_err(|error| error.to_string())?;
    if status.get("status").and_then(Value::as_str) == Some("unauthenticated") {
        return Err("Bitwarden is signed out on this machine: run `bw login` in a terminal".into());
    }
    let session = cli
        .run(&["unlock", "--passwordenv", "BW_PASSWORD", "--raw"], &[("BW_PASSWORD", password)])
        .map_err(|error| format!("could not unlock Bitwarden: {error}"))?;
    let session = session.trim().to_owned();
    if session.is_empty() {
        return Err("could not unlock Bitwarden".into());
    }
    if let Err(error) = cli.run(&["sync"], &[("BW_SESSION", &session)]) {
        tracing::debug!(%error, "bitwarden sync failed");
    }
    Ok(session)
}

pub fn folder_items(cli: &dyn BitwardenCli, session: &str, folder: &str) -> Result<Vec<Item>, String> {
    let env = [("BW_SESSION", session)];
    let folders = cli.run(&["list", "folders", "--search", folder], &env)?;
    let id = folder_id(&folders, folder)?.ok_or_else(|| format!("no Bitwarden folder is named `{folder}`"))?;
    let items = cli.run(&["list", "items", "--folderid", &id], &env)?;
    parse_items(&items)
}

pub(super) fn folder_id(json: &str, name: &str) -> Result<Option<String>, String> {
    let folders: Vec<Value> = serde_json::from_str(json).map_err(|error| error.to_string())?;
    Ok(folders
        .iter()
        .find(|f| f.get("name").and_then(Value::as_str).is_some_and(|n| n.eq_ignore_ascii_case(name.trim())))
        .and_then(|f| f.get("id").and_then(Value::as_str))
        .map(str::to_owned))
}

pub fn parse_items(json: &str) -> Result<Vec<Item>, String> {
    let items: Vec<Value> = serde_json::from_str(json).map_err(|error| error.to_string())?;
    Ok(items.iter().filter_map(item).collect())
}

fn item(value: &Value) -> Option<Item> {
    let name = value.get("name").and_then(Value::as_str)?.to_owned();
    let mut values = BTreeMap::new();
    let mut keep = |field: &str, text: Option<&str>| {
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            values.entry(field.to_owned()).or_insert_with(|| text.to_owned());
        }
    };
    let login = value.get("login");
    keep("username", login.and_then(|l| l.get("username")).and_then(Value::as_str));
    keep("password", login.and_then(|l| l.get("password")).and_then(Value::as_str));
    keep("notes", value.get("notes").and_then(Value::as_str));
    for field in value.get("fields").and_then(Value::as_array).into_iter().flatten() {
        if let Some(field_name) = field.get("name").and_then(Value::as_str) {
            keep(field_name, field.get("value").and_then(Value::as_str));
        }
    }
    let uris = login.and_then(|l| l.get("uris")).and_then(Value::as_array).into_iter().flatten().filter_map(uri);
    Some(Item { name, values, uris: uris.collect() })
}

fn uri(value: &Value) -> Option<LoginUri> {
    let text = value.get("uri").and_then(Value::as_str).filter(|t| !t.trim().is_empty())?;
    let rule = match value.get("match").and_then(Value::as_u64) {
        Some(0) => Some(UriRule::Domain),
        Some(1) => Some(UriRule::Host),
        Some(2) => Some(UriRule::StartsWith),
        Some(3) => Some(UriRule::Exact),
        Some(4) => Some(UriRule::RegularExpression),
        Some(5) => Some(UriRule::Never),
        _ => None,
    };
    Some(LoginUri::new(text, rule))
}
