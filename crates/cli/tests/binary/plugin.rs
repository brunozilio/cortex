use std::path::Path;
use std::process::Command;

use crate::{Sandbox, command};

fn git(sandbox: &Sandbox, dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", sandbox.home())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn local_plugin_repo(sandbox: &Sandbox) -> Option<std::path::PathBuf> {
    let repo = sandbox.path("sources/ship-it");
    let manifest = repo.join(format!(".{}-plugin", command())).join("plugin.json");
    sandbox.write(&manifest, r#"{"name":"ship-it","description":"Ships the release"}"#);
    let made = git(sandbox, &repo, &["init", "-q"])
        && git(sandbox, &repo, &["add", "."])
        && git(sandbox, &repo, &["commit", "-q", "-m", "plugin"]);
    made.then_some(repo)
}

#[test]
fn with_no_plugins_the_list_says_how_to_install_one() {
    let sandbox = Sandbox::fresh();
    let hint = format!("No plugins installed. `{} plugin install <git-url>` adds one.", command());
    sandbox.run(&["plugin"]).succeeded().quiet_on_stderr().said(&hint);
    sandbox.run(&["plugin", "list"]).succeeded().quiet_on_stderr().said(&hint);
}

#[test]
fn a_plugin_is_installed_from_a_local_repository_updated_listed_and_removed() {
    let sandbox = Sandbox::fresh();
    let Some(repo) = local_plugin_repo(&sandbox) else {
        eprintln!("git is not available; the install is not exercised");
        return;
    };
    let source = repo.display().to_string();
    let installed = sandbox.run(&["plugin", "install", &source]);
    installed.succeeded().said("Installed ship-it. A new chat or /reload picks it up.");
    assert!(sandbox.user_dir().join("plugins/ship-it").is_dir());

    let listed = sandbox.run(&["plugin", "list"]);
    listed.succeeded();
    assert_eq!(listed.stdout, format!("{:<24} Ships the release\n", "ship-it"));

    sandbox.run(&["plugin", "update", "ship-it"]).succeeded().said("Updated ship-it:");

    let again = sandbox.run(&["plugin", "install", &source]);
    again.failed_with(1).quiet_on_stdout().complained("ship-it is already installed; update or remove it");

    sandbox.run(&["plugin", "remove", "ship-it"]).succeeded().said("Removed ship-it.");
    assert!(!sandbox.user_dir().join("plugins/ship-it").exists());
    sandbox.run(&["plugin", "list"]).said("No plugins installed.");
}

#[test]
fn a_plugin_can_be_installed_under_another_name() {
    let sandbox = Sandbox::fresh();
    let Some(repo) = local_plugin_repo(&sandbox) else {
        eprintln!("git is not available; the install is not exercised");
        return;
    };
    let ran = sandbox.run(&["plugin", "install", "--name", "renamed", &repo.display().to_string()]);
    ran.succeeded().said("Installed renamed.");
    assert!(sandbox.user_dir().join("plugins/renamed").is_dir());
}

#[test]
fn installing_from_a_path_that_is_not_a_repository_fails_naming_it() {
    let sandbox = Sandbox::fresh();
    let missing = sandbox.path("nowhere/plugin");
    let ran = sandbox.run(&["plugin", "install", &missing.display().to_string()]);
    ran.failed_with(1).quiet_on_stdout().complained(&missing.display().to_string());
    sandbox.run(&["plugin", "list"]).said("No plugins installed.");
}

#[test]
fn removing_or_updating_a_plugin_that_is_not_there_fails_naming_it() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["plugin", "remove", "ghost"]).failed_with(1).quiet_on_stdout().complained("no plugin called ghost");
    sandbox
        .run(&["plugin", "update", "ghost"])
        .failed_with(1)
        .quiet_on_stdout()
        .complained("ghost is not a git checkout here, so it cannot be updated");
}

#[test]
fn install_needs_a_source() {
    let ran = Sandbox::fresh().run(&["plugin", "install"]);
    ran.failed_with(2).quiet_on_stdout().complained("<SOURCE>");
}
