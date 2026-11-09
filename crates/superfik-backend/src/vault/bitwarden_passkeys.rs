//! Passkeys in `bw list items` output: each login's `fido2Credentials`.
//! Bitwarden writes some of their fields as text (`"discoverable":
//! "true"`), so both shapes are read.

use serde_json::Value;

use super::bitwarden::BitwardenCli;
use super::provider::{KeptPasskey, VaultPasskey};

pub fn parse_passkeys(json: &str) -> Result<Vec<VaultPasskey>, String> {
    let items: Vec<Value> = serde_json::from_str(json).map_err(|error| error.to_string())?;
    Ok(items.iter().flat_map(item_passkeys).collect())
}

fn item_passkeys(item: &Value) -> Vec<VaultPasskey> {
    let Some(name) = item.get("name").and_then(Value::as_str) else { return Vec::new() };
    let login = item.get("login");
    let credentials = login.and_then(|l| l.get("fido2Credentials")).and_then(Value::as_array);
    credentials.into_iter().flatten().filter_map(|c| passkey(name, login, c)).collect()
}

fn passkey(item: &str, login: Option<&Value>, value: &Value) -> Option<VaultPasskey> {
    let text = |field: &str| value.get(field).and_then(Value::as_str).filter(|t| !t.trim().is_empty());
    let user_name = text("userName").or_else(|| login?.get("username")?.as_str()).map(str::to_owned);
    let discoverable = match value.get("discoverable") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
        _ => false,
    };
    Some(VaultPasskey {
        item: item.to_owned(),
        credential: text("credentialId")?.to_owned(),
        rp_id: text("rpId")?.to_owned(),
        user_name,
        user_handle: text("userHandle").map(str::to_owned),
        discoverable,
        key: text("keyValue")?.to_owned(),
    })
}

pub fn keep_passkey(cli: &dyn BitwardenCli, session: &str, folder: &str, kept: &KeptPasskey) -> Result<(), String> {
    use base64::Engine;
    let env = [("BW_SESSION", session)];
    let passkey = &kept.passkey;
    let site = format!("https://{}", passkey.rp_id);
    let listed: Vec<Value> =
        serde_json::from_str(&cli.run(&["list", "items", "--url", &site], &env)?).map_err(|error| error.to_string())?;
    let user = passkey.user_name.as_deref().unwrap_or_default();
    let existing = listed.iter().find(|item| {
        let name = item["login"]["username"].as_str().unwrap_or_default();
        !user.is_empty() && name.eq_ignore_ascii_case(user)
    });
    let encode = |item: &Value| base64::engine::general_purpose::STANDARD.encode(item.to_string());
    if let Some(id) = existing.and_then(|item| item["id"].as_str()) {
        let mut item: Value =
            serde_json::from_str(&cli.run(&["get", "item", id], &env)?).map_err(|error| error.to_string())?;
        let login = item.get_mut("login").ok_or("the login has no sign-in")?;
        match login.get_mut("fido2Credentials").and_then(Value::as_array_mut) {
            Some(list) => list.push(credential(kept)),
            None => login["fido2Credentials"] = Value::Array(vec![credential(kept)]),
        }
        return cli.run_with_input(&["edit", "item", id], &env, &encode(&item)).map(|_| ());
    }
    let folders = cli.run(&["list", "folders", "--search", folder], &env)?;
    let name = if kept.rp_name.is_empty() { passkey.rp_id.clone() } else { kept.rp_name.clone() };
    let item = serde_json::json!({
        "type": 1,
        "name": name,
        "folderId": super::bitwarden::folder_id(&folders, folder)?,
        "organizationId": null,
        "notes": null,
        "favorite": false,
        "reprompt": 0,
        "fields": [],
        "login": {
            "username": passkey.user_name,
            "password": null,
            "uris": [{ "uri": site, "match": null }],
            "fido2Credentials": [credential(kept)],
        },
    });
    cli.run_with_input(&["create", "item"], &env, &encode(&item)).map(|_| ())
}

fn credential(kept: &KeptPasskey) -> Value {
    let p = &kept.passkey;
    serde_json::json!({
        "credentialId": p.credential,
        "keyType": "public-key",
        "keyAlgorithm": "ECDSA",
        "keyCurve": "P-256",
        "keyValue": p.key,
        "rpId": p.rp_id,
        "userHandle": p.user_handle,
        "userName": p.user_name,
        "counter": "0",
        "rpName": kept.rp_name,
        "userDisplayName": kept.user_display_name,
        "discoverable": if p.discoverable { "true" } else { "false" },
        "creationDate": kept.created,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorded {
        calls: Mutex<Vec<(String, Option<String>)>>,
    }

    impl BitwardenCli for Recorded {
        fn run(&self, args: &[&str], _: &[(&str, &str)]) -> Result<String, String> {
            self.calls.lock().expect("calls").push((args.join(" "), None));
            Ok(match args {
                ["list", "items", ..] => r#"[{"id":"i1","login":{"username":"Bruno"}}]"#.into(),
                ["get", "item", "i1"] => r#"{"id":"i1","login":{"username":"Bruno","fido2Credentials":null}}"#.into(),
                ["list", "folders", ..] => r#"[{"id":"f1","name":"Superfik"}]"#.into(),
                _ => String::new(),
            })
        }

        fn run_with_input(&self, args: &[&str], _: &[(&str, &str)], input: &str) -> Result<String, String> {
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD.decode(input).expect("base64");
            self.calls.lock().expect("calls").push((args.join(" "), Some(String::from_utf8(decoded).expect("utf8"))));
            Ok(String::new())
        }
    }

    fn kept(user: &str) -> KeptPasskey {
        KeptPasskey {
            passkey: VaultPasskey {
                item: String::new(),
                credential: "b5b9b2a4-4c26-4b8d-9f6e-1f0a2b3c4d5e".into(),
                rp_id: "github.com".into(),
                user_name: Some(user.into()),
                user_handle: Some("dXNlcg".into()),
                discoverable: true,
                key: "a2V5".into(),
            },
            rp_name: "GitHub".into(),
            user_display_name: "Bruno".into(),
            created: "2026-10-31T22:00:00.000Z".into(),
        }
    }

    #[test]
    fn a_passkey_joins_the_login_of_its_account_on_stdin() {
        let cli = Recorded::default();
        keep_passkey(&cli, "s", "Superfik", &kept("bruno")).expect("kept");
        let calls = cli.calls.lock().expect("calls");
        let (args, input) = calls.last().expect("edited");
        assert_eq!(args, "edit item i1");
        let item: Value = serde_json::from_str(input.as_deref().expect("stdin")).expect("json");
        let credential = &item["login"]["fido2Credentials"][0];
        assert_eq!((credential["rpId"].as_str(), credential["keyValue"].as_str()), (Some("github.com"), Some("a2V5")));
        assert_eq!(credential["discoverable"], "true");
        assert!(calls.iter().all(|(args, _)| !args.contains("a2V5")), "the key never on the command line");
    }

    #[test]
    fn a_passkey_for_another_account_gets_a_login_of_its_own() {
        let cli = Recorded::default();
        keep_passkey(&cli, "s", "Superfik", &kept("ana")).expect("kept");
        let calls = cli.calls.lock().expect("calls");
        let (args, input) = calls.last().expect("created");
        assert_eq!(args, "create item");
        let item: Value = serde_json::from_str(input.as_deref().expect("stdin")).expect("json");
        assert_eq!((item["name"].as_str(), item["folderId"].as_str()), (Some("GitHub"), Some("f1")));
        assert_eq!(item["login"]["uris"][0]["uri"], "https://github.com");
        assert_eq!(item["login"]["fido2Credentials"][0]["userName"], "ana");
    }

    #[test]
    fn passkeys_are_read_from_each_logins_fido2_credentials() {
        let json = serde_json::json!([
            { "name": "Google", "login": { "username": "ana@example.com", "fido2Credentials": [
                { "credentialId": "b5b9b2a4-4c26-4b8d-9f6e-1f0a2b3c4d5e", "rpId": "google.com",
                  "userHandle": "dXNlcg", "keyValue": "a2V5", "discoverable": "true", "counter": "0" },
                { "credentialId": "b64.AQID", "rpId": "google.com", "keyValue": "a2V5", "discoverable": false,
                  "userName": "other" },
                { "credentialId": "no-key", "rpId": "google.com" }
            ] } },
            { "name": "Note", "notes": "n" }
        ])
        .to_string();
        let found = parse_passkeys(&json).expect("parses");
        assert_eq!(found.len(), 2, "the one without a key is skipped");
        assert_eq!(found[0].item, "Google");
        assert_eq!(found[0].user_name.as_deref(), Some("ana@example.com"), "the login's username");
        assert_eq!(found[0].user_handle.as_deref(), Some("dXNlcg"));
        assert!(found[0].discoverable);
        assert_eq!((found[1].user_name.as_deref(), found[1].discoverable), (Some("other"), false));
        assert!(!format!("{found:?}").contains("a2V5"), "debug hides the key");
    }
}
