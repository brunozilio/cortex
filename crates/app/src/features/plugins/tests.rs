use std::process::Command;

use super::*;

fn repository(at: &Path) {
    let run =
        |args: &[&str]| assert!(Command::new("git").args(args).current_dir(at).output().expect("git").status.success());
    std::fs::create_dir_all(at.join(concat!(".", brand::command!(), "-plugin"))).expect("folder");
    std::fs::write(at.join(concat!(".", brand::command!(), "-plugin/plugin.json")), r#"{"description":"Ships"}"#)
        .expect("manifest");
    run(&["init", "-q"]);
    run(&["add", "."]);
    run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "first"]);
}

#[test]
fn a_plugin_is_installed_listed_updated_and_removed() {
    let source = tempfile::tempdir().expect("source");
    let repo = source.path().join("ship-it.git");
    std::fs::create_dir_all(&repo).expect("repo");
    repository(&repo);
    let home = tempfile::tempdir().expect("home");
    let folder = home.path().join("plugins");

    let installed = install(&folder, repo.to_str().expect("path"), None).expect("installed");
    assert_eq!(installed, PluginsReply::Installed { name: "ship-it".into(), at: folder.join("ship-it") });
    assert_eq!(list(&folder), [Plugin { name: "ship-it".into(), description: "Ships".into() }]);
    assert!(install(&folder, repo.to_str().expect("path"), None).is_err(), "not twice");
    assert!(matches!(update(&folder, "ship-it"), Ok(PluginsReply::Updated { name, .. }) if name == "ship-it"));
    assert_eq!(remove(&folder, "ship-it"), Ok(PluginsReply::Removed { name: "ship-it".into() }));
    assert!(list(&folder).is_empty());
}

#[test]
fn plugins_need_a_home_and_a_blank_source_or_name_is_none_at_all() {
    let home = tempfile::tempdir().expect("home");
    let mut services = crate::features::context::testing::services(None);
    let homeless =
        crate::features::context::testing::with(1, &services, |context| handle(context, PluginsRequest::List));
    assert!(homeless.is_err(), "nowhere to keep plugins");
    services.homes = std::sync::Arc::new(crate::OwnerHome::at(home.path()));
    let plugins =
        |request: PluginsRequest| crate::features::context::testing::with(1, &services, |c| handle(c, request));

    assert_eq!(plugins(PluginsRequest::List), Ok(PluginsReply::Listed(Vec::new())));
    let blank = plugins(PluginsRequest::Install { source: "  ".into(), name: None });
    assert_eq!(blank, Err("install needs a git URL or path".into()));
    assert_eq!(plugins(PluginsRequest::Update { name: " ".into() }), Err("update needs a plugin's name".into()));
    assert_eq!(plugins(PluginsRequest::Remove { name: String::new() }), Err("remove needs a plugin's name".into()));
    let unnamed = plugins(PluginsRequest::Install { source: "/".into(), name: Some("  ".into()) });
    assert!(unnamed.is_err_and(|error| error.contains("pass --name")), "a blank name is taken from the source");
}

#[cfg(unix)]
#[test]
fn updating_a_plugin_never_lets_git_ask_at_the_terminal() {
    let home = tempfile::tempdir().expect("home");
    let folder = home.path().join("plugins");
    let target = folder.join("asks");
    std::fs::create_dir_all(&target).expect("target");
    repository(&target);
    let remote = home.path().join("remote.sh");
    std::fs::write(&remote, "echo \"prompt=${GIT_TERMINAL_PROMPT:-unset}\" >&2\nexit 1\n").expect("remote");
    let run = |args: &[&str]| {
        assert!(Command::new("git").args(args).current_dir(&target).output().expect("git").status.success());
    };
    run(&["config", "core.sshCommand", &format!("sh {}", remote.display())]);
    run(&["remote", "add", "origin", "ssh://example.invalid/acme/asks.git"]);

    let refused = update(&folder, "asks").expect_err("the remote fails");

    assert!(refused.contains("prompt=0"), "{refused}");
}

#[test]
fn a_plugin_s_own_config_cannot_turn_a_remote_into_a_command() {
    let home = tempfile::tempdir().expect("home");
    let folder = home.path().join("plugins");
    let target = folder.join("runs");
    std::fs::create_dir_all(&target).expect("target");
    repository(&target);
    let marker = home.path().join("ran");
    let run = |args: &[&str]| {
        assert!(Command::new("git").args(args).current_dir(&target).output().expect("git").status.success());
    };
    run(&["config", "protocol.ext.allow", "always"]);
    run(&["remote", "add", "origin", &format!("ext::sh -c touch% {}", marker.display())]);

    let refused = update(&folder, "runs").expect_err("refused");

    assert!(refused.contains("'ext' not allowed"), "{refused}");
    assert!(!marker.exists());
}

#[test]
fn names_come_from_the_source_and_stay_inside_the_folder() {
    assert_eq!(named_after("git@github.com:acme/ship-it.git").as_deref(), Some("ship-it"));
    assert_eq!(named_after("https://github.com/acme/tools/").as_deref(), Some("tools"));
    let folder = Path::new("/home/a/plugins");
    assert!(plugin(folder, "../escape").is_err());
    assert!(plugin(folder, ".hidden").is_err());
    assert!(remove(folder, "missing").is_err());
}
