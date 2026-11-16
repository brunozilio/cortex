use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use domain::permission_rules::{PermissionRules, RuleKind};
use platform::sync::Locked;

use super::{Models, Policy, Tracking, Workspace};
use crate::guard::Safety;
use crate::tools::read_files::Freshness;
use crate::tools::script_tool::ScriptAccess;

fn rules(text: &str) -> PermissionRules {
    PermissionRules::default().with(RuleKind::Allow, text)
}

#[test]
fn a_forked_policy_keeps_permissions_and_scripts_but_clears_pre_approved() {
    let policy = Policy {
        permissions: rules("Bash(git status)"),
        pre_approved: rules("Bash(cargo test)"),
        scripts: ScriptAccess::none(),
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
        web: domain::web_reading::WebReading::none(),
        output: domain::output::Shaping { cleanup: true, filters: false, savings: true },
    };

    let fork = policy.fork();

    assert_eq!(fork.web, policy.web, "a sub-agent reads the web as its parent does");
    assert_eq!(fork.output, policy.output, "a sub-agent's command output is shaped as its parent's");
    assert_eq!(fork.sandbox, policy.sandbox, "a sub-agent's commands stay as confined as its parent's");
    assert!(fork.redact.is_some(), "a sub-agent's output is redacted like its parent's");
    assert!(fork.snapshots && fork.dependency_guard);
    assert_eq!(fork.permissions, policy.permissions);
    assert_eq!(fork.scripts, ScriptAccess::none());
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
fn a_forked_workspace_keeps_models_and_drops_verification_and_worktree() {
    let mut workspace = Workspace::new("/repo");
    workspace.models = Models { small_model: Some("small".into()), ..Models::default() };
    workspace.verification = Some(super::Verification {
        criteria: domain::Criteria::user("tests pass"),
        model: "judge".into(),
        allow: PermissionRules::default(),
    });

    let fork = workspace.fork(Arc::new(AtomicBool::new(false)));

    assert_eq!(fork.models.small_model.as_deref(), Some("small"));
    assert_eq!(fork.cwd, Path::new("/repo"));
    assert!(fork.verification.is_none());
    assert!(fork.worktree.is_none());
}
