use std::collections::BTreeMap;

use superfik_core::VaultKind;
use superfik_core::autofill::LoginUri;

use super::super::provider::VaultProvider;
use super::*;

#[derive(Default)]
struct FakeVault {
    unlocks: Mutex<u32>,
    locked: Mutex<bool>,
    syncs: Mutex<u32>,
}

impl VaultProvider for FakeVault {
    fn refresh(&self, _account: &VaultAccount, _session: &str) -> Result<(), String> {
        *self.syncs.lock().expect("syncs") += 1;
        Ok(())
    }

    fn items(&self, account: &VaultAccount, session: &mut Option<String>) -> Result<Vec<Item>, String> {
        if account.secrets.get("password").map(String::as_str) != Some("hunter2") {
            return Err("could not unlock".into());
        }
        let mut locked = self.locked.lock().expect("locked");
        match session {
            Some(_) if *locked => return Err("Vault is locked.".into()),
            Some(_) => {}
            None => {
                *self.unlocks.lock().expect("unlocks") += 1;
                *locked = false;
                *session = Some("s1".into());
            }
        }
        let login = |name: &str, uri: &str, user: &str| Item {
            name: name.into(),
            values: BTreeMap::from([("username".into(), user.into()), ("password".into(), format!("{name}-pw"))]),
            uris: vec![LoginUri::new(uri, None)],
        };
        let note = Item { name: "Note".into(), values: BTreeMap::from([("notes".into(), "n".into())]), uris: vec![] };
        Ok(vec![login("GitHub", "https://github.com", "bruno"), login("Linear", "https://linear.app", "b"), note])
    }

    fn logins(&self, account: &VaultAccount, session: &mut Option<String>, _url: &str) -> Result<Vec<Item>, String> {
        let mut items = self.items(account, session)?;
        items.push(Item {
            name: "Bank".into(),
            values: BTreeMap::from([("username".into(), "b".into()), ("password".into(), "Bank-pw".into())]),
            uris: vec![LoginUri::new("https://bank.com", None)],
        });
        Ok(items)
    }
}

fn account(password: &str) -> VaultAccount {
    VaultAccount {
        title: format!("Bitwarden · {password}"),
        kind: VaultKind::Bitwarden,
        scope: "Superfik".into(),
        secrets: crate::plugins::Secrets::from([("password".to_owned(), password.to_owned())]),
    }
}

fn logins() -> (Logins, Arc<FakeVault>) {
    let vault = Arc::new(FakeVault::default());
    let providers = VaultProviders::default().with(VaultKind::Bitwarden, vault.clone());
    (Logins::new(providers), vault)
}

const OWNER: Principal = Principal::LocalOwner;

#[test]
fn candidates_are_the_logins_saved_for_the_page_without_passwords() {
    let (logins, _) = logins();
    let found = logins.candidates(&OWNER, &[account("hunter2")], "https://gist.github.com/x").expect("found");
    assert_eq!(
        found,
        [Candidate { item: "GitHub".into(), account: "Bitwarden · hunter2".into(), username: Some("bruno".into()) }]
    );
    assert!(logins.candidates(&OWNER, &[account("hunter2")], "https://example.com").expect("none").is_empty());
    assert!(logins.candidates(&OWNER, &[], "https://github.com").expect("no vault").is_empty());
}

#[test]
fn one_locked_account_does_not_hide_another() {
    let (logins, _) = logins();
    let both = [account("wrong"), account("hunter2")];
    assert_eq!(logins.candidates(&OWNER, &both, "https://github.com").expect("found").len(), 1);
    let only_wrong = [account("wrong")];
    assert!(logins.candidates(&OWNER, &only_wrong, "https://github.com").is_err_and(|e| e.contains("unlock")));
}

#[test]
fn a_fill_gives_the_values_only_for_a_page_the_login_belongs_to() {
    let (logins, _) = logins();
    let accounts = [account("hunter2")];
    let fill = logins.fill(&OWNER, &accounts, "https://github.com/login", "Bitwarden · hunter2", "GitHub", false);
    assert_eq!(fill, Ok(LoginFill { username: Some("bruno".into()), password: Some("GitHub-pw".into()) }));
    let elsewhere = logins.fill(&OWNER, &accounts, "https://evil.com/login", "Bitwarden · hunter2", "GitHub", false);
    assert!(elsewhere.is_err_and(|e| !e.contains("GitHub-pw")));
    let unknown = logins.fill(&OWNER, &accounts, "https://github.com", "Bitwarden · other", "GitHub", false);
    assert!(unknown.is_err());
}

#[test]
fn the_user_reaches_the_whole_vault_and_an_agent_only_the_folder() {
    let (logins, _) = logins();
    let accounts = [account("hunter2")];
    let found = logins.candidates(&OWNER, &accounts, "https://bank.com/login").expect("found");
    assert_eq!(found.iter().map(|c| c.item.as_str()).collect::<Vec<_>>(), ["Bank"], "outside any folder");
    let user = logins.fill(&OWNER, &accounts, "https://bank.com/login", "Bitwarden · hunter2", "Bank", false);
    assert!(user.is_ok());
    let agent = logins.fill(&OWNER, &accounts, "https://bank.com/login", "Bitwarden · hunter2", "Bank", true);
    assert!(agent.is_err(), "an agent keeps to the account's folder");
}

#[test]
fn two_accounts_on_one_vault_list_a_login_once() {
    let (logins, _) = logins();
    let other = VaultAccount { title: "Bitwarden · Pessoal".into(), ..account("hunter2") };
    let found = logins.candidates(&OWNER, &[account("hunter2"), other], "https://github.com").expect("found");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].account, "Bitwarden · hunter2");
}

#[test]
fn the_session_is_kept_and_unlocked_again_once_locked() {
    let (logins, vault) = logins();
    let accounts = [account("hunter2")];
    logins.candidates(&OWNER, &accounts, "https://github.com").expect("first");
    logins.candidates(&OWNER, &accounts, "https://github.com").expect("second");
    assert_eq!(*vault.unlocks.lock().expect("unlocks"), 1, "kept between fills");
    *vault.locked.lock().expect("locked") = true;
    logins.candidates(&OWNER, &accounts, "https://github.com").expect("after locking");
    assert_eq!(*vault.unlocks.lock().expect("unlocks"), 2);
}

#[test]
fn a_kept_session_syncs_once_it_is_old_enough() {
    let (logins, vault) = logins();
    let accounts = [account("hunter2")];
    logins.candidates(&OWNER, &accounts, "https://github.com").expect("first");
    logins.candidates(&OWNER, &accounts, "https://github.com").expect("second");
    assert_eq!(*vault.syncs.lock().expect("syncs"), 0, "fresh: unlocking already synced");
    let logins = logins.syncing_every(Duration::ZERO);
    logins.candidates(&OWNER, &accounts, "https://github.com").expect("third");
    assert_eq!(*vault.syncs.lock().expect("syncs"), 1);
    assert_eq!(*vault.unlocks.lock().expect("unlocks"), 1, "syncing keeps the session");
}
