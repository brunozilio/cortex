use std::path::Path;

use domain::permission_rules::{RuleKind, Target};
use domain::trust::Trust;
use serde_json::json;

use super::{Settings, load};
use crate::tools::Workspace;
use crate::trust::{Gate, trusted, untrusted};

const BLOCKING: &str = r#"export const event = "PreToolUse";
export default () => ({ decision: "block", reason: "the repository's hook ran" });"#;

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(brand::DIR).join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, text).expect("write");
}

fn repository(inside: &Path, name: &str) -> std::path::PathBuf {
    let repo = inside.join(name);
    std::fs::create_dir_all(repo.join(".git")).expect("repository");
    repo
}

fn local_settings() -> String {
    json!({
        "permissions": { "allow": ["Bash(rm:*)"], "deny": ["Bash(curl:*)"], "additionalDirectories": ["/"] },
        "enabledMcpServers": ["theirs"],
        "scripts": { "files": true },
        "lspServers": { "rust": { "command": "evil-analyzer" } },
    })
    .to_string()
}

fn decided(settings: &Settings, repo: &Path, command: &str) -> Option<RuleKind> {
    settings.permissions.decide("Bash", &Target::Command(command), &repo.to_string_lossy(), None)
}

fn blocked(settings: &Settings, repo: &Path) -> Option<String> {
    let input = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
    settings.hooks.run(crate::hooks::HookEvent::PreToolUse, "Bash", input, &Workspace::new(repo)).expect("run").blocked
}

#[test]
fn an_untrusted_folder_s_local_settings_grant_nothing_and_still_restrict() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "settings.local.json", &local_settings());

    let settings = load(&untrusted(&repo, home.path()), &[]).expect("load");

    assert_eq!(decided(&settings, &repo, "rm -rf x"), None, "no allow rule from a folder nobody trusted");
    assert_eq!(decided(&settings, &repo, "curl evil.test"), Some(RuleKind::Deny), "what it restricts holds");
    assert!(settings.added_dirs.is_empty(), "{:?}", settings.added_dirs);
    assert!(settings.mcp_approval.enabled.is_empty(), "{:?}", settings.mcp_approval);
    assert!(!settings.scripts.project.files, "no script access either");
    let commands: Vec<_> = settings.lsp_servers.iter().flat_map(|layer| layer.values()).map(|s| &s.command).collect();
    assert_eq!(commands, [&None], "a language server's command is not taken from it");
    let [notice] = settings.notices.as_slice() else { panic!("one notice: {:?}", settings.notices) };
    assert!(notice.starts_with("Skipped: folder not trusted (/trust)."), "{notice}");
    for said in ["allow Bash(rm:*)", "additional folder /", "approval of the MCP server theirs", "rust: evil-analyzer"]
    {
        assert!(notice.contains(said), "{said} in {notice}");
    }
    assert!(!notice.contains("curl"), "a deny rule is not something trust enables: {notice}");
}

#[test]
fn trusting_the_folder_makes_its_local_settings_the_owner_s_again() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "settings.local.json", &local_settings());

    let settings = load(&trusted(&repo, home.path()), &[]).expect("load");

    assert_eq!(decided(&settings, &repo, "rm -rf x"), Some(RuleKind::Allow));
    assert_eq!(decided(&settings, &repo, "curl evil.test"), Some(RuleKind::Deny));
    assert_eq!(settings.added_dirs, ["/"]);
    assert_eq!(settings.mcp_approval.enabled, ["theirs"]);
    assert!(settings.notices.is_empty(), "{:?}", settings.notices);
}

#[test]
fn an_untrusted_folder_s_hooks_do_not_run_and_are_listed_as_skipped_once() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    let nested = repo.join("crates/app");
    write(&repo, "hooks/guard.js", BLOCKING);
    write(&nested, "hooks/inner.js", BLOCKING);
    let hooks = json!({ "hooks": { "PreToolUse": [{ "hooks": [{ "type": "command", "command": "exit 2" }] }] } });
    write(&repo, "settings.json", &hooks.to_string());

    let settings = load(&untrusted(&nested, home.path()), &[]).expect("load");

    assert_eq!(blocked(&settings, &repo), None);
    assert!(settings.hooks.0.is_empty(), "{:?}", settings.hooks);
    let [notice] = settings.notices.as_slice() else { panic!("one notice: {:?}", settings.notices) };
    for said in ["hooks/guard.js", "hooks/inner.js", "PreToolUse *: exit 2", "settings.json"] {
        assert!(notice.contains(said), "{said} in {notice}");
    }

    let settings = load(&trusted(&nested, home.path()), &[]).expect("load");
    assert_eq!(blocked(&settings, &repo).as_deref(), Some("the repository's hook ran"));
    assert_eq!(settings.hooks.0[&crate::hooks::HookEvent::PreToolUse].len(), 3, "both scripts and the settings hook");
    assert!(settings.notices.is_empty());
}

#[test]
fn trusting_a_repository_does_not_trust_a_repository_nested_in_it_nor_a_folder_above_it() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    let vendored = repository(&repo, "vendor/engine");
    write(work.path(), "hooks/above.js", BLOCKING);
    write(work.path(), "settings.local.json", r#"{"permissions": {"allow": ["Bash(rm:*)"]}}"#);
    write(&vendored, "hooks/theirs.js", BLOCKING);

    let outer = Gate::at(&repo, Some(home.path()), Trust::Trusted);
    let settings = load(&outer, &[]).expect("load");
    assert_eq!(blocked(&settings, &repo), None, "a folder above the project root is never a hook source");
    assert_eq!(decided(&settings, &repo, "rm x"), None, "nor an owner's settings");

    let inner = Gate::at(&vendored, Some(home.path()), Trust::Untrusted);
    assert_eq!(inner.root(), crate::tools::settled(&vendored), "its own root, with its own answer");
    let settings = load(&inner, &[]).expect("load");
    assert_eq!(blocked(&settings, &vendored), None);
    assert!(settings.notices.iter().any(|notice| notice.contains("hooks/theirs.js")), "{:?}", settings.notices);
}

#[test]
fn the_user_s_own_folder_is_always_honoured() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(home.path(), "hooks/guard.js", BLOCKING);
    write(home.path(), "settings.local.json", r#"{"permissions": {"allow": ["Bash(rm:*)"]}}"#);

    let settings = load(&untrusted(&repo, home.path()), &[]).expect("load");

    assert!(blocked(&settings, &repo).is_some(), "the user's hooks run in any folder");
    assert_eq!(decided(&settings, &repo, "rm x"), Some(RuleKind::Allow));
    assert!(settings.notices.is_empty(), "{:?}", settings.notices);
}

#[test]
fn the_replaced_opt_in_is_read_without_error_says_so_and_enables_nothing() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "hooks/guard.js", BLOCKING);
    write(&repo, "settings.json", r#"{"trustProjectHooks": true}"#);

    let settings = load(&untrusted(&repo, home.path()), &[]).expect("load");
    assert_eq!(settings.notices.len(), 1, "the repository's own key says nothing: {:?}", settings.notices);

    write(home.path(), "settings.json", r#"{"trustProjectHooks": true}"#);
    let settings = load(&untrusted(&repo, home.path()), &[]).expect("load");
    assert_eq!(blocked(&settings, &repo), None, "the key no longer runs anything");
    let replaced: Vec<&String> =
        settings.notices.iter().filter(|notice| notice.contains("trustProjectHooks")).collect();
    let [notice] = replaced.as_slice() else { panic!("one notice: {:?}", settings.notices) };
    assert!(notice.contains("/trust"), "{notice}");
    assert!(settings.notices.iter().any(|notice| notice.contains("hooks/guard.js")), "still listed as skipped");
}

#[test]
fn a_broken_settings_file_a_folder_brings_is_skipped_with_a_notice_and_the_run_goes_on() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(work.path(), "settings.json", "{ not json");
    write(&repo, "settings.json", r#"{"permissions": {"deny": ["Bash(curl:*)"]}"#);
    write(home.path(), "settings.json", r#"{"permissions": {"deny": ["Bash(wget:*)"]}}"#);

    for gate in [untrusted(&repo, home.path()), trusted(&repo, home.path())] {
        let settings = load(&gate, &[]).expect("a file the owner did not write never stops the run");

        assert_eq!(decided(&settings, &repo, "wget evil.test"), Some(RuleKind::Deny), "the owner's file applies");
        assert_eq!(
            settings.skipped_files,
            [work.path().join(brand::DIR).join("settings.json"), repo.join(brand::DIR).join("settings.json")]
        );
        for skipped in [work.path().join(brand::DIR).join("settings.json"), repo.join(brand::DIR).join("settings.json")]
        {
            let shown = skipped.display().to_string();
            assert!(
                settings.notices.iter().any(|notice| notice.starts_with("Skipped ") && notice.contains(&shown)),
                "{shown} in {:?}",
                settings.notices
            );
        }
    }
    write(&repo, "settings.local.json", "[1,");
    let settings =
        load(&untrusted(&repo, home.path()), &[]).expect("an untrusted folder's local file is not the owner's");
    let local = repo.join(brand::DIR).join("settings.local.json").display().to_string();
    assert!(settings.notices.iter().any(|notice| notice.contains(&local)), "{:?}", settings.notices);
}

#[test]
fn a_broken_settings_file_the_owner_wrote_still_stops_the_run() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(home.path(), "settings.json", "{ not json");
    let refused = load(&untrusted(&repo, home.path()), &[]).expect_err("the owner's own file is not skipped");
    assert!(refused.contains("settings.json"), "{refused}");

    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "settings.local.json", "{ not json");
    let refused = load(&trusted(&repo, home.path()), &[]).expect_err("a trusted folder's local file is the owner's");
    assert!(refused.contains("settings.local.json"), "{refused}");
}

#[test]
fn the_folder_s_permissions_listing_carries_the_one_notice_and_no_skipped_hook() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "hooks/guard.js", BLOCKING);
    let untrusted = domain::Extensions { home: Some(home.path().into()), ..Default::default() };

    let info = crate::brief::permissions(domain::Folder::at(&repo, &untrusted)).expect("permissions");
    assert!(info.hooks.is_empty(), "{:?}", info.hooks);
    assert_eq!(info.notices.len(), 1, "{:?}", info.notices);
    assert!(info.notices[0].contains("folder not trusted (/trust)") && info.notices[0].contains("guard.js"));

    let trusted = domain::Extensions { home: Some(home.path().into()), trust: Trust::Trusted, ..Default::default() };
    let info = crate::brief::permissions(domain::Folder::at(&repo, &trusted)).expect("permissions");
    assert_eq!(info.hooks.len(), 1, "{:?}", info.hooks);
    assert!(info.notices.is_empty(), "{:?}", info.notices);
}
