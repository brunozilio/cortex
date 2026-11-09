use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::config::Config;

fn args(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn config(dir: &tempfile::TempDir) -> Config {
    Config { listen: crate::config::DEFAULT_LISTEN.parse().expect("addr"), data: dir.path().join("data") }
}

#[test]
fn commands_are_read_with_their_defaults() {
    assert_eq!(parse(&[]), Ok(Command::Serve));
    assert_eq!(parse(&args("init")), Ok(Command::Init { owner: "admin".into() }));
    assert_eq!(parse(&args("init --owner olga")), Ok(Command::Init { owner: "olga".into() }));
    assert_eq!(parse(&args("user add ana")), Ok(Command::AddUser { name: "ana".into(), role: OrgRole::Member }));
    assert_eq!(
        parse(&args("user add ana --role Admin")),
        Ok(Command::AddUser { name: "ana".into(), role: OrgRole::Admin })
    );
    assert_eq!(parse(&args("config domain -")), Ok(Command::Domain(None)));
    assert_eq!(
        parse(&args("install --domain superfik.acme.io")),
        Ok(Command::Install { domain: Some("superfik.acme.io".into()), user: "superfik".into() })
    );
    assert!(parse(&args("user add ana --role boss")).is_err());
    assert!(parse(&args("frobnicate")).is_err());
}

#[test]
fn init_adds_the_owner_once_and_users_follow() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let said = run(&Command::Init { owner: "olga".into() }, &config).expect("init");
    assert!(said.contains("First-access code for olga"), "{said}");
    let again = run(&Command::Init { owner: "olga".into() }, &config).expect("again");
    assert!(again.contains("set up already"), "{again}");
    run(&Command::AddUser { name: "ana".into(), role: OrgRole::Admin }, &config).expect("add");
    let listed = run(&Command::Users, &config).expect("list");
    assert!(listed.contains("ana") && listed.contains("admin") && listed.contains("pending"), "{listed}");
    run(&Command::RemoveUser { name: "ana".into() }, &config).expect("remove");
    assert!(!run(&Command::Users, &config).expect("list").contains("ana"));
    assert!(run(&Command::RemoveUser { name: "olga".into() }, &config).is_err(), "the last owner stays");
}

#[test]
fn a_code_made_here_opens_the_running_server() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let server = organization(&config, Arc::new(|| 10)).expect("server");
    let said = run(&Command::Init { owner: "olga".into() }, &config).expect("init");
    let code = said.split("olga: ").nth(1).and_then(|rest| rest.split_whitespace().next()).expect("code");
    let token = server.directory.first_access("olga", code, "correct horse battery", 10).expect("first access");
    assert!(crate::Authenticator::authenticate(&server, &token).is_some());
}

#[test]
fn the_domain_is_kept_and_cleared() {
    let dir = tempfile::tempdir().expect("dir");
    let config = config(&dir);
    let shown = run(&Command::Domain(Some("superfik.acme.io".into())), &config).expect("domain");
    assert!(shown.contains("Domain: superfik.acme.io"), "{shown}");
    let cleared = run(&Command::Domain(None), &config).expect("clear");
    assert!(cleared.contains("Domain: (none)"), "{cleared}");
}

#[test]
fn install_takes_a_host_name_and_a_system_user_or_nothing() {
    assert_eq!(parse(&args("install")), Ok(Command::Install { domain: None, user: "superfik".into() }));
    assert_eq!(
        parse(&args("install --user svc --domain Superfik.Acme.io")),
        Ok(Command::Install { domain: Some("superfik.acme.io".into()), user: "svc".into() })
    );
    for bad in [
        "install --domain",
        "install --domain --user svc",
        "install --user",
        "install --domain acme.io;reboot",
        "install --domain localhost",
        "install --user root",
        "install --user superfik-runner",
        "install --user a;b",
        "install --domain a.io --domain b.io",
        "install --force",
        "install extra",
        "init --owner",
        "user add ana --role",
        "config domain acme.io/../x",
    ] {
        assert!(parse(&args(bad)).is_err(), "{bad}");
    }
    assert_eq!(parse(&args("config domain Acme.io")), Ok(Command::Domain(Some("acme.io".into()))));
}

#[test]
fn only_the_data_folders_owner_manages_it() {
    let data = Path::new("/var/lib/superfik");
    assert_eq!(as_data_owner(Some(998), Some(998), data), Ok(()));
    assert_eq!(as_data_owner(None, Some(998), data), Ok(()), "unknown here (not Linux)");
    assert_eq!(as_data_owner(Some(0), None, data), Ok(()), "no folder yet");
    let said = as_data_owner(Some(0), Some(998), data).expect_err("root");
    assert!(said.contains("sudo -u '#998'") && said.contains("/var/lib/superfik"), "{said}");
    assert!(as_data_owner(Some(1000), Some(998), data).is_err());
}
