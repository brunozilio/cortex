use std::path::Path;

use domain::permission_rules::{RuleKind, Target};
use serde_json::json;

use super::super::settings::{Settings, load};
use crate::tools::Workspace;

const BLOCKING: &str = r#"export const event = "PreToolUse";
export default () => ({ decision: "block", reason: "the repository's hook ran" });"#;

fn git(repo: &Path, args: &[&str]) {
    let done = std::process::Command::new("git").arg("-C").arg(repo).args(args).output().expect("git runs");
    assert!(done.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&done.stderr));
}

fn repository() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repo");
    git(repo.path(), &["init", "-q"]);
    repo
}

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(brand::DIR).join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, text).expect("write");
}

fn local_settings() -> String {
    json!({
        "permissions": { "allow": ["Bash(rm:*)"], "additionalDirectories": ["/"] },
        "enabledMcpServers": ["theirs"],
    })
    .to_string()
}

fn allows_rm(settings: &Settings, repo: &Path) -> bool {
    let cwd = repo.to_string_lossy();
    settings.permissions.decide("Bash", &Target::Command("rm -rf x"), &cwd, None) == Some(RuleKind::Allow)
}

fn blocked(settings: &Settings, repo: &Path) -> Option<String> {
    let input = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    settings.hooks.run("PreToolUse", "Bash", input, &Workspace::new(repo)).expect("run").blocked
}

#[test]
fn a_local_settings_file_the_repository_versions_grants_nothing() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    write(repo.path(), "settings.local.json", &local_settings());
    git(repo.path(), &["add", "-A"]);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(!allows_rm(&settings, repo.path()), "a committed file is the repository's, not the owner's");
    assert!(settings.added_dirs.is_empty(), "{:?}", settings.added_dirs);
    assert!(settings.mcp_approval.enabled.is_empty(), "{:?}", settings.mcp_approval);
}

#[test]
fn a_local_settings_file_left_out_of_git_is_still_the_owner_s() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    write(repo.path(), "settings.local.json", &local_settings());
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(allows_rm(&settings, repo.path()));
    assert_eq!(settings.added_dirs, ["/"]);

    let plain = tempfile::tempdir().expect("no repository");
    write(plain.path(), "settings.local.json", &local_settings());
    let settings = load(plain.path(), Some(home.path()), &[]).expect("load");
    assert!(allows_rm(&settings, plain.path()), "outside a repository nothing changes");
}

#[test]
fn a_hook_script_the_repository_versions_does_not_run_and_the_run_is_told() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    write(repo.path(), "hooks/guard.js", BLOCKING);
    git(repo.path(), &["add", "-A"]);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert_eq!(blocked(&settings, repo.path()), None);
    let [notice] = settings.notices.as_slice() else { panic!("one notice: {:?}", settings.notices) };
    assert!(notice.contains("guard.js") && notice.contains("trustProjectHooks"), "{notice}");
    assert!(notice.contains("workspace trust"), "{notice}");
}

#[test]
fn the_folder_s_permissions_listing_carries_the_notice_about_skipped_hooks() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    write(repo.path(), "hooks/guard.js", BLOCKING);
    git(repo.path(), &["add", "-A"]);
    let mut request = domain::PromptRequest::plain(repo.path().into(), "", domain::ModelOption::new("m", "M"));
    request.extensions = Some(domain::Extensions { home: Some(home.path().into()), ..Default::default() });
    let info = crate::brief::permissions(&request).expect("permissions");
    assert!(info.hooks.is_empty(), "{:?}", info.hooks);
    assert!(info.notices.iter().any(|notice| notice.contains("guard.js")), "{:?}", info.notices);
}

#[test]
fn hooks_in_a_settings_file_the_repository_versions_are_not_registered() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    let hooks = json!({ "hooks": { "PreToolUse": [{ "hooks": [{ "type": "command", "command": "exit 2" }] }] },
        "permissions": { "deny": ["Bash(rm:*)"] } });
    write(repo.path(), "settings.json", &hooks.to_string());
    git(repo.path(), &["add", "-A"]);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(settings.hooks.0.is_empty(), "{:?}", settings.hooks);
    assert!(settings.notices.iter().any(|notice| notice.contains("settings.json")), "{:?}", settings.notices);
    let cwd = repo.path().to_string_lossy();
    let denied = settings.permissions.decide("Bash", &Target::Command("rm x"), &cwd, None);
    assert_eq!(denied, Some(RuleKind::Deny), "what the versioned file restricts still holds");
}

#[test]
fn the_owner_s_opt_in_runs_the_repository_s_versioned_hooks_as_before() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    write(repo.path(), "hooks/guard.js", BLOCKING);
    write(repo.path(), "settings.json", r#"{"trustProjectHooks": true}"#);
    git(repo.path(), &["add", "-A"]);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert_eq!(blocked(&settings, repo.path()), None, "the repository cannot opt itself in");

    write(home.path(), "settings.json", r#"{"trustProjectHooks": true}"#);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert_eq!(blocked(&settings, repo.path()).as_deref(), Some("the repository's hook ran"));
    assert!(settings.notices.is_empty(), "{:?}", settings.notices);
}

#[test]
fn a_project_hook_left_out_of_git_and_the_user_s_own_hooks_still_run() {
    let (home, repo) = (tempfile::tempdir().expect("home"), repository());
    write(repo.path(), "hooks/guard.js", BLOCKING);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(blocked(&settings, repo.path()).is_some(), "untracked: the owner's working copy");
    assert!(settings.notices.is_empty());

    let (home, repo) = (repository(), repository());
    write(home.path(), "hooks/guard.js", BLOCKING);
    git(home.path(), &["add", "-A"]);
    let settings = load(repo.path(), Some(home.path()), &[]).expect("load");
    assert!(blocked(&settings, repo.path()).is_some(), "the user's folder is theirs even when they version it");
}
