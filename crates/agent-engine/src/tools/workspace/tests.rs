use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::permission_rules::{PermissionRules, RuleKind};
use platform::sync::Locked;

use super::{Models, Policy, Tracking, Workspace};
use crate::guard::Safety;
use crate::tools::read_files::Freshness;
use crate::tools::script_tool::ScriptGrants;

fn rules(text: &str) -> PermissionRules {
    PermissionRules::default().with(RuleKind::Allow, text)
}

#[test]
fn a_forked_policy_keeps_permissions_and_scripts_but_clears_pre_approved() {
    let policy = Policy {
        permissions: rules("Bash(git status)"),
        pre_approved: rules("Bash(cargo test)"),
        scripts: ScriptGrants::none(),
        trust: domain::trust::Trust::Trusted,
        safety: None,
        sandbox: Some(domain::sandbox::Sandbox { network: false, ..Default::default() }),
        redact: Some(Arc::new(domain::redact::Known::default())),
        snapshots: true,
        dependency_guard: true,
        escalation: true,
        auto_review: true,
        follow_through: true,
        affected_tests: true,
        session_claims: true,
        load_on_call: true,
        code_mode: domain::code_mode::Mode::Only,
        code_mode_detach: Some(std::time::Duration::from_secs(5)),
        web: domain::web_reading::WebReading::none(),
        output: domain::output::Shaping { cleanup: true, filters: false, savings: true },
        bash_output: domain::output::OutputLimit::of(4_000),
    };

    let fork = policy.fork();

    assert_eq!(fork.trust, policy.trust, "a sub-agent works in a folder as trusted as its parent's");
    assert_eq!(fork.web, policy.web, "a sub-agent reads the web as its parent does");
    assert_eq!(fork.output, policy.output, "a sub-agent's command output is shaped as its parent's");
    assert_eq!(fork.bash_output, policy.bash_output, "a sub-agent's command output is cut as its parent's");
    assert_eq!(fork.sandbox, policy.sandbox, "a sub-agent's commands stay as confined as its parent's");
    assert!(fork.redact.is_some(), "a sub-agent's output is redacted like its parent's");
    assert!(fork.snapshots && fork.dependency_guard);
    assert!(fork.load_on_call, "a sub-agent loads a waiting tool on call as its parent does");
    assert_eq!(fork.code_mode, policy.code_mode, "a sub-agent scripts its tools as its parent does");
    assert_eq!(fork.permissions, policy.permissions);
    assert_eq!(fork.scripts, ScriptGrants::none());
    assert!(fork.pre_approved.is_empty());
    assert!(fork.safety.is_none());
}

#[test]
fn a_forked_policy_gets_its_own_copy_of_the_safety_check() {
    let safety = Arc::new(Safety::default());
    let policy = Policy { safety: Some(Arc::clone(&safety)), ..Policy::default() };

    let fork = policy.fork();

    let forked = fork.safety.as_ref().map(|forked| Arc::ptr_eq(forked, &safety));
    assert_eq!(forked, Some(false));
}

#[test]
fn a_forked_tracking_shares_what_was_read() {
    let tracking = Tracking::default();
    let fork = tracking.fork();
    let path = PathBuf::from("/repo/missing-on-purpose.rs");

    fork.mark_read(&path);

    assert!(Arc::ptr_eq(&tracking.read, &fork.read));
    assert_ne!(tracking.freshness(&path), Freshness::Unread);
}

#[test]
fn a_forked_tracking_starts_with_no_refusals_seen_files_or_seen_rules() {
    let tracking = Tracking::default();
    for _ in 0..domain::interception::MAX_REFUSALS_IN_A_ROW {
        tracking.count_refusal(true);
    }
    tracking.seen_rules.locked().insert(PathBuf::from("/repo/.rules/a.md"));

    let fork = tracking.fork();

    assert!(tracking.refusals_stop().is_some());
    assert_eq!(fork.refusals_stop(), None);
    assert!(fork.seen_rules.locked().is_empty());
    assert!(!Arc::ptr_eq(&tracking.seen_rules, &fork.seen_rules));
    assert!(!Arc::ptr_eq(&tracking.seen_files, &fork.seen_files));
}

#[test]
fn a_repository_s_config_and_hooks_are_guarded_once_it_exists_so_a_command_can_still_make_one() {
    let (fresh, repo) = (tempfile::tempdir().expect("fresh"), tempfile::tempdir().expect("repo"));
    std::fs::create_dir(repo.path().join(".git")).expect("repository");
    let sandbox = domain::sandbox::Sandbox::default();
    let guarded = |project: &Path| {
        let places = domain::sandbox::Places {
            project,
            added: &[],
            repository: None,
            temp: &[],
            home: None,
            settings_dir: brand::DIR,
        };
        super::confined::confinement_of(&sandbox, &places).protected
    };

    assert_eq!(guarded(fresh.path()), [fresh.path().join(brand::DIR)], "git init may write .git/config");
    let guarded = guarded(repo.path());
    for path in [brand::DIR, ".git/config", ".git/config.worktree", ".git/hooks"] {
        assert!(guarded.contains(&repo.path().join(path)), "{path}: {guarded:?}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn a_sandboxed_command_cannot_replace_a_toolchain_or_an_unpacked_crate_and_package_managers_still_cache() {
    let (project, home) = (tempfile::tempdir().expect("project"), tempfile::tempdir().expect("home"));
    let temp = [project.path().join("tmp")];
    std::fs::create_dir(&temp[0]).expect("temp");
    let planted = |name: &str, text: &str| {
        let path = home.path().join(name);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(&path, text).expect("file");
        path
    };
    let cargo = planted(".rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo", "#!/bin/sh\n");
    let source = planted(".cargo/registry/src/index.crates.io-0/serde-1.0.0/src/lib.rs", "pub fn ok() {}\n");
    let archive = planted(".cargo/registry/cache/index.crates.io-0/serde-1.0.1.crate", "archive");
    let installed = planted(".cargo/bin/cargo-nextest", "#!/bin/sh\n");
    let hook = planted(".cache/pre-commit/repo1/hook.py", "print()\n");
    let lock = home.path().join(".cargo/.package-cache");
    let places = domain::sandbox::Places {
        project: project.path(),
        added: &[],
        repository: None,
        temp: &temp,
        home: Some(home.path()),
        settings_dir: brand::DIR,
    };
    let confinement = super::confined::confinement_of(&domain::sandbox::Sandbox::default(), &places);
    let runs = |code: &str| {
        let mut shell = platform::sandbox::confined(platform::script_shell(code), &confinement);
        shell.current_dir(project.path()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        shell.status().expect("the sandbox starts").success()
    };

    for path in [&cargo, &source, &archive, &installed, &hook] {
        assert!(!runs(&format!("echo evil >> '{}'", path.display())), "{} was written", path.display());
        assert!(!runs(&format!("rm '{}'", path.display())), "{} was removed", path.display());
    }
    assert!(!runs(&format!("mkdir '{}/.rustup/toolchains/evil'", home.path().display())));
    assert_eq!(std::fs::read_to_string(&cargo).expect("cargo"), "#!/bin/sh\n");
    assert_eq!(std::fs::read_to_string(&source).expect("source"), "pub fn ok() {}\n");
    assert!(runs(&format!("touch '{}'", lock.display())), "cargo still takes its package cache lock");
    assert!(runs(r#"mkdir -p "$npm_config_cache/_cacache" && echo x > "$npm_config_cache/_cacache/probe""#));
    assert!(runs(r#"mkdir -p "$GOMODCACHE" && echo x > "$GOMODCACHE/probe""#));
    assert!(domain::sandbox::own_caches(&temp[0]).join("npm/_cacache/probe").is_file());
}

#[test]
fn a_forked_workspace_keeps_models_and_drops_verification_and_worktree() {
    let mut workspace = Workspace::new("/repo");
    workspace.models = Models { small_model: Some("small".into()), ..Models::default() };
    workspace.verification = Some(super::Verification {
        criteria: domain::Criteria::user("tests pass"),
        model: "judge".into(),
        allow: PermissionRules::default(),
    });

    let fork = workspace.fork(Arc::default());

    assert_eq!(fork.models.small_model.as_deref(), Some("small"));
    assert_eq!(fork.cwd, Path::new("/repo"));
    assert!(fork.verification.is_none());
    assert!(fork.worktree.is_none());
}
