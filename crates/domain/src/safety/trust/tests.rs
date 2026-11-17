use std::path::Path;

use super::*;
use crate::approval::{AskKind, Decision};

const HOME: &str = "/users/ana";

fn repositories(dir: &Path) -> bool {
    ["/users/ana/work/rocket", "/users/ana/work/rocket/vendor/engine", "/users/ana"]
        .contains(&dir.to_string_lossy().as_ref())
}

fn none(_: &Path) -> bool {
    false
}

fn home() -> Option<&'static Path> {
    Some(Path::new(HOME))
}

fn trusting(root: &str) -> TrustedFolders {
    let mut folders = TrustedFolders::default();
    folders.trust(Path::new(root), home(), 7).expect("trusted");
    folders
}

#[test]
fn a_folder_s_trust_root_is_its_repository_or_itself() {
    let root = |folder: &str| trust_root(Path::new(folder), home(), &repositories);
    assert_eq!(root("/users/ana/work/rocket/src/bin"), Path::new("/users/ana/work/rocket"));
    assert_eq!(root("/users/ana/work/rocket"), Path::new("/users/ana/work/rocket"));
    assert_eq!(root("/users/ana/work/rocket/vendor/engine/src"), Path::new("/users/ana/work/rocket/vendor/engine"));
    assert_eq!(root("/users/ana/notes/2026"), Path::new("/users/ana/notes/2026"), "a home kept in git is no project");
    assert_eq!(trust_root(Path::new("/srv/plain/sub"), home(), &none), Path::new("/srv/plain/sub"));
    assert_eq!(project_root(Path::new("/users/ana/notes"), &repositories), Path::new(HOME), "the plain rule");
}

#[test]
fn trusting_a_repository_trusts_its_folders_and_nothing_around_it() {
    let folders = trusting("/users/ana/work/rocket");
    let of = |folder: &str| folders.of(Path::new(folder), home(), &repositories);
    assert_eq!(of("/users/ana/work/rocket"), Trust::Trusted);
    assert_eq!(of("/users/ana/work/rocket/src/bin"), Trust::Trusted);
    assert_eq!(of("/users/ana/work"), Trust::Untrusted, "not its parent");
    assert_eq!(of("/users/ana/work/rocket-2"), Trust::Untrusted, "not a folder whose name starts the same");
    assert_eq!(of("/users/ana/work/rocket/vendor/engine"), Trust::Untrusted, "not a repository nested in it");
    assert_eq!(of("/users/ana/work/rocket/vendor/engine/src"), Trust::Untrusted);
}

#[test]
fn a_folder_outside_any_repository_is_trusted_alone() {
    let folders = trusting("/srv/plain");
    assert_eq!(folders.of(Path::new("/srv/plain"), home(), &none), Trust::Trusted);
    assert_eq!(folders.of(Path::new("/srv/plain/sub"), home(), &none), Trust::Untrusted);
    assert_eq!(folders.of(Path::new("/srv"), home(), &none), Trust::Untrusted);
}

#[test]
fn the_home_folder_and_the_file_system_root_are_never_trusted() {
    let mut folders = TrustedFolders::default();
    assert_eq!(folders.trust(Path::new(HOME), home(), 1), Err(Refusal::Home));
    assert_eq!(folders.trust(Path::new("/"), home(), 1), Err(Refusal::FilesystemRoot));
    assert!(folders.listed().is_empty());
    assert!(Refusal::Home.to_string().contains("home folder"));
    assert_eq!(folders.trust(Path::new(HOME), None, 1), Ok(()), "without a known home it is a folder like another");
}

#[test]
fn trust_is_kept_once_with_its_first_date_and_can_be_taken_back() {
    let mut folders = trusting("/srv/plain");
    folders.trust(Path::new("/srv/plain"), home(), 99).expect("again");
    assert_eq!(folders.listed(), [TrustedFolder { path: "/srv/plain".into(), trusted_at: 7 }]);
    let kept = serde_json::to_string(&folders).expect("json");
    assert_eq!(kept, r#"[{"path":"/srv/plain","trusted_at":7}]"#);
    assert_eq!(serde_json::from_str::<TrustedFolders>(&kept).expect("read"), folders);
    folders.untrust(Path::new("/srv/plain"));
    assert_eq!(folders.of_root(Path::new("/srv/plain")), Trust::Untrusted);
    assert_eq!(Trust::default(), Trust::Untrusted, "nothing said is not trusted");
}

fn report(trust: Trust, items: &[(ContributionKind, &str)]) -> TrustReport {
    let mut brings = Contributions::default();
    for (kind, what) in items {
        brings.add(*kind, *what);
    }
    TrustReport { root: "/srv/rocket".into(), trust, refusal: None, brings }
}

#[test]
fn what_a_folder_brings_is_listed_by_kind_in_one_notice() {
    let items = [
        (ContributionKind::McpServer, "jira"),
        (ContributionKind::HookScript, "hooks/guard.js"),
        (ContributionKind::McpServer, "sonar"),
        (ContributionKind::McpServer, "jira"),
    ];
    let report = report(Trust::Untrusted, &items);
    assert_eq!(report.brings.len(), 3, "the same thing is said once");
    assert_eq!(
        report.brings.skipped_notice().expect("notice"),
        "Skipped: folder not trusted (/trust).\n  Hook scripts:\n    hooks/guard.js\n  MCP servers:\n    jira\n    sonar"
    );
    assert_eq!(report.brings.counted(), "Hook scripts (1), MCP servers (2)");
    assert!(report.skipped_line().expect("line").contains("3 project items were skipped: Hook scripts (1)"));
    assert!(report.skipped_line().expect("line").contains("--trust"));
    assert_eq!(Contributions::default().skipped_notice(), None);
}

#[test]
fn only_an_untrusted_folder_that_brings_something_and_can_be_trusted_asks() {
    let hook = [(ContributionKind::HookScript, "hooks/guard.js")];
    assert!(report(Trust::Untrusted, &hook).asks());
    assert!(!report(Trust::Untrusted, &[]).asks(), "nothing to agree to");
    assert!(!report(Trust::Trusted, &hook).asks());
    assert_eq!(report(Trust::Trusted, &hook).skipped_line(), None);
    let refused = TrustReport { refusal: Some(Refusal::Home), ..report(Trust::Untrusted, &hook) };
    assert!(!refused.asks());
    assert!(refused.state().contains("home folder"), "{}", refused.state());
    assert!(report(Trust::Untrusted, &[]).described().ends_with("brings nothing that trust would enable."));
    assert!(
        report(Trust::Trusted, &hook).described().starts_with("/srv/rocket is trusted.\nIt brings:\n  Hook scripts")
    );
}

#[test]
fn the_dialog_names_the_folder_and_what_it_enables_and_continuing_is_the_default() {
    let asked = ask(&report(Trust::Untrusted, &[(ContributionKind::HookScript, "hooks/guard.js")]));
    assert_eq!(asked.id.0, ASK);
    let AskKind::Questions(questions) = &asked.kind else { panic!("not a question") };
    assert!(questions[0].text.contains("/srv/rocket") && questions[0].text.contains("hooks/guard.js"));
    let labels: Vec<&str> = questions[0].options.iter().map(|option| option.label.as_str()).collect();
    assert_eq!(labels, ["Trust this folder", "Continue without trusting", "Exit"]);
    assert_eq!(labels[DEFAULT_CHOICE], "Continue without trusting");
    assert_eq!(answer(&Decision::Answer(vec![vec![0]])), TrustAnswer::Trust);
    assert_eq!(answer(&Decision::Answer(vec![vec![1]])), TrustAnswer::Continue);
    assert_eq!(answer(&Decision::Answer(vec![vec![2]])), TrustAnswer::Exit);
    assert_eq!(answer(&Decision::Deny), TrustAnswer::Continue, "closing the question trusts nothing");
    assert_eq!(answer(&Decision::Answer(Vec::new())), TrustAnswer::Continue);
}

#[test]
fn a_long_list_is_cut_in_the_dialog_and_says_where_the_rest_is() {
    let scripts: Vec<String> = (0..40).map(|at| format!("hooks/h{at}.js")).collect();
    let items: Vec<(ContributionKind, &str)> =
        scripts.iter().map(|script| (ContributionKind::HookScript, script.as_str())).collect();
    let long = report(Trust::Untrusted, &items);
    let AskKind::Questions(questions) = &ask(&long).kind else { panic!("not a question") };
    let shown = &questions[0].text;
    assert!(shown.contains("hooks/h0.js") && !shown.contains("hooks/h39.js"), "{shown}");
    assert!(shown.contains("and 29 more (/trust lists everything)"), "{shown}");
    assert_eq!(long.brings.listing().lines().count(), 41, "the full listing is not cut");
    let short = report(Trust::Untrusted, &items[..3]);
    assert_eq!(short.brings.listing_within(12), short.brings.listing());
}

#[test]
fn the_replaced_setting_s_notice_points_at_trusting_the_folder() {
    let notice = legacy_notice();
    assert!(notice.contains(LEGACY_OPT_IN) && notice.contains("/trust") && notice.contains("--trust"), "{notice}");
}
