use std::path::{Path, PathBuf};

use domain::code_mode::Mode;
use domain::memory::{Author, Memory, MemoryBrief, MemoryId, MemoryKind};
use domain::{Criteria, ModelOption, PermissionMode};
use serde_json::json;

use super::*;

fn settings(flags: serde_json::Value) -> Settings {
    Settings { features: Features::default().layered(&flags, true).expect("flags"), ..Settings::default() }
}

fn policy(flags: serde_json::Value) -> Policy {
    flagged(&settings(flags), Path::new("/nowhere"), false)
}

fn request() -> PromptRequest {
    PromptRequest::plain(PathBuf::from("/nowhere"), "go", ModelOption::new("m", "M"))
}

type Reads = fn(&Policy) -> bool;

const SWITCHES: [(&str, Reads); 20] = [
    (features::WEB_LINKS, |policy| policy.web.links),
    (features::WEB_SMALL_PAGES_DIRECT, |policy| policy.web.small_pages_direct),
    (features::WEB_FORMATS, |policy| policy.web.formats),
    (features::WEB_EMPTY_PAGE_ERROR, |policy| policy.web.empty_page_error),
    (features::SANDBOX, |policy| policy.sandbox.is_some()),
    (features::REDACT_SECRETS, |policy| policy.redact.is_some()),
    (features::DESTRUCTIVE_SNAPSHOT, |policy| policy.snapshots),
    (features::DEPENDENCY_GUARD, |policy| policy.dependency_guard),
    (features::ESCALATION, |policy| policy.escalation),
    (features::AUTO_REVIEW, |policy| policy.auto_review),
    (features::FOLLOW_THROUGH, |policy| policy.follow_through),
    (features::AFFECTED_TESTS, |policy| policy.affected_tests),
    (features::SESSION_CLAIMS, |policy| policy.session_claims),
    (features::LOAD_ON_CALL, |policy| policy.load_on_call),
    (features::CODE_MODE, |policy| policy.code_mode != Mode::Off),
    (features::WEB_MAIN_CONTENT, |policy| policy.web.main_content),
    (features::WEB_CLEANUP, |policy| policy.web.cleanup),
    (features::OUTPUT_CLEANUP, |policy| policy.output.cleanup),
    (features::OUTPUT_FILTERS, |policy| policy.output.filters),
    (features::OUTPUT_SAVINGS, |policy| policy.output.savings),
];

#[test]
fn with_no_flag_set_every_switch_is_on_and_the_repl_is_the_only_tool() {
    let policy = policy(json!({}));
    for (flag, reads) in SWITCHES {
        assert!(reads(&policy), "{flag} is on until it is turned off");
    }
    assert_eq!(policy.code_mode, Mode::Only);
    assert_eq!(policy.web, domain::web_reading::WebReading::from(&Features::default()));
}

#[test]
fn turning_one_flag_off_turns_off_its_own_switch_and_no_other() {
    for (flag, _) in SWITCHES {
        let policy = policy(json!({ flag: false }));
        for (other, reads) in SWITCHES {
            assert_eq!(reads(&policy), other != flag, "{other} with {flag} off");
        }
    }
}

#[test]
fn the_sandbox_is_the_one_the_settings_describe() {
    let settings = settings(json!({}));
    let policy = flagged(&settings, Path::new("/nowhere"), false);
    assert_eq!(policy.sandbox, Some(settings.sandbox.clone()));
    assert_eq!(policy.scripts, settings.scripts);
}

#[test]
fn turning_code_mode_off_gives_the_agent_its_tools_back() {
    assert_eq!(policy(json!({ features::CODE_MODE: false })).code_mode, Mode::Off);
}

#[test]
fn secrets_are_read_from_the_folder_s_env_files_only_when_redaction_is_on() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join(".env"), "TOKEN=s3cr3t-value-123456\n").expect("env");
    let said = "the token is s3cr3t-value-123456";
    let redacting = flagged(&settings(json!({})), dir.path(), false);
    assert!(!redacting.redacted(said).contains("s3cr3t-value-123456"));
    let plain = flagged(&settings(json!({ features::REDACT_SECRETS: false })), dir.path(), false);
    assert_eq!(plain.redacted(said), said);
}

#[test]
fn the_safety_classifier_follows_the_request_and_no_flag() {
    let settings = settings(json!({}));
    assert!(flagged(&settings, Path::new("/nowhere"), false).safety.is_none());
    assert!(flagged(&settings, Path::new("/nowhere"), true).safety.is_some());
}

#[test]
fn flags_grant_no_permission_rule_and_no_pre_approval() {
    let policy = policy(json!({}));
    assert_eq!(policy.permissions, PermissionRules::default());
    assert_eq!(policy.pre_approved, PermissionRules::default());
    assert_eq!(policy.code_mode_detach, None);
}

#[test]
fn a_budget_is_kept_only_while_its_flag_is_on() {
    let mut request = request();
    request.policy.budget = Some(domain::budget::Budget { limit_usd: 2.0, spent_usd: 0.5 });
    assert_eq!(budget(&request, &Features::default()), request.policy.budget);
    assert_eq!(budget(&request, &settings(json!({ features::BUDGET: false })).features), None);
}

#[test]
fn rules_asked_on_the_command_line_come_after_the_settings_rules() {
    let mut settings = Settings::default();
    settings.permissions.add(RuleKind::Allow, "Read").expect("rule");
    let mut request = request();
    request.policy.allowed_tools = vec!["Bash(ls:*)".into()];
    request.policy.ask_tools = vec!["Edit".into()];
    request.policy.disallowed_tools = vec!["WebFetch".into()];
    let rules = permissions(&request, &settings).expect("rules");
    assert_eq!(
        rules.listed(),
        [
            (RuleKind::Deny, "WebFetch".to_owned()),
            (RuleKind::Ask, "Edit".to_owned()),
            (RuleKind::Allow, "Read".to_owned()),
            (RuleKind::Allow, "Bash(ls:*)".to_owned()),
        ]
    );
}

#[test]
fn a_rule_that_does_not_parse_names_the_flags_it_came_from() {
    let mut request = request();
    request.policy.disallowed_tools = vec!["Bash(".into()];
    let error = permissions(&request, &Settings::default()).expect_err("unparsed");
    assert!(error.starts_with("--allowedTools/--disallowedTools: "), "{error}");
}

#[test]
fn nothing_is_verified_in_plan_mode() {
    let mut request = request();
    request.policy.verification = Some(Criteria::project("tests pass"));
    request.policy.permission = PermissionMode::Plan;
    assert_eq!(verification(&request, &Settings::default(), &ModelRoles::default()), Ok(None));
}

#[test]
fn the_project_s_criteria_apply_only_with_their_flag_and_yield_to_the_request_s() {
    let project = |flags| Settings { project_criteria: Some("lint is clean".into()), ..settings(flags) };
    let request = request();
    let judged = verification(&request, &project(json!({})), &ModelRoles::default()).expect("verification");
    assert_eq!(judged.map(|found| found.criteria), Some(Criteria::project("lint is clean")));
    let off = project(json!({ features::PROJECT_CRITERIA: false }));
    assert_eq!(verification(&request, &off, &ModelRoles::default()), Ok(None));
    let mut asking = request.clone();
    asking.policy.verification = Some(Criteria::project("tests pass"));
    let judged = verification(&asking, &project(json!({})), &ModelRoles::default()).expect("verification");
    assert_eq!(judged.map(|found| found.criteria.text), Some("tests pass".to_owned()));
}

#[test]
fn the_judge_is_the_review_role_s_model_or_the_run_s_own() {
    let mut request = request();
    request.policy.verification = Some(Criteria::project("tests pass"));
    let own = verification(&request, &Settings::default(), &ModelRoles::default()).expect("verification");
    assert_eq!(own.map(|found| found.model), Some("m".to_owned()));
    let mut roles = ModelRoles::default();
    roles.set(ModelRole::Review, Some("no such model anywhere".into()));
    let error = verification(&request, &Settings::default(), &roles).expect_err("unknown judge");
    assert!(error.starts_with("judgeModel: "), "{error}");
}

fn anchor() -> Memory {
    Memory {
        id: MemoryId(7),
        kind: MemoryKind::Semantic,
        text: "Keep defaultRetryMax at 4".into(),
        tags: Vec::new(),
        created: 0,
        origin: None,
        author: Author::User,
        history: Vec::new(),
        uses: 0,
        last_used: 0,
        touches: vec!["client.go".into()],
        credit: None,
    }
}

#[test]
fn anchored_memories_need_the_brief_to_ask_and_the_flag_to_be_on() {
    let mut request = request();
    let brief = |anchors| MemoryBrief { anchors, anchored: vec![anchor()], ..MemoryBrief::default() };
    request.host.memory = Some(brief(true));
    assert_eq!(anchored(&request, &Features::default()), [anchor()]);
    let off = settings(json!({ features::MEMORY_ANCHORS: false })).features;
    assert_eq!(anchored(&request, &off), []);
    request.host.memory = Some(brief(false));
    assert_eq!(anchored(&request, &Features::default()), []);
}

struct Unboxed;

impl crate::tools::Launcher for Unboxed {
    fn launch(
        &self,
        command: std::process::Command,
        _: Option<&platform::sandbox::Confinement>,
    ) -> std::process::Command {
        command
    }

    fn support(&self) -> platform::sandbox::Support {
        platform::sandbox::Support::Missing("this system has no sandbox".into())
    }
}

#[test]
fn a_sandbox_the_system_cannot_provide_is_said_once_per_process() {
    let said = AtomicBool::new(false);
    let mut workspace = Workspace::new("/nowhere");
    workspace.services.launch = crate::tools::Launch(Arc::new(Unboxed));
    assert_eq!(sandbox_notice(&workspace, &said), None, "the sandbox is off: nothing to say");
    workspace.policy.sandbox = Some(domain::sandbox::Sandbox { required: false, ..Default::default() });
    let notice = sandbox_notice(&workspace, &said).expect("said");
    assert!(notice.contains("this system has no sandbox") && notice.contains("unconfined"), "{notice}");
    assert_eq!(sandbox_notice(&workspace, &said), None, "once is enough");
}
