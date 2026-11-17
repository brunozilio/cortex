use crate::{Sandbox, command};

fn line<'a>(report: &'a str, check: &str) -> &'a str {
    report
        .lines()
        .find(|line| line.trim_start().split_once(' ').is_some_and(|(_, rest)| rest.starts_with(check)))
        .unwrap_or_else(|| panic!("no {check} line in:\n{report}"))
}

fn passed(report: &str, check: &str) -> bool {
    line(report, check).trim_start().starts_with('✓')
}

#[test]
fn a_clean_home_reports_every_check_and_fails_only_for_the_missing_provider() {
    let sandbox = Sandbox::fresh();
    let ran = sandbox.run(&["doctor"]);
    ran.failed_with(1).quiet_on_stderr();
    let report = ran.stdout.as_str();
    assert!(report.lines().next().is_some_and(|first| first.ends_with("problem(s):")), "{ran:?}");
    assert!(line(report, "version").ends_with("0.0.1"));
    assert!(!passed(report, "providers"));
    assert!(line(report, "providers").contains(&format!("run `{} login`", command())));
    assert!(!passed(report, "models"));
    for check in ["version", "settings", "trust", "mcp servers", "declared providers", "sandbox", "ripgrep"] {
        assert!(passed(report, check), "{check} should pass: {ran:?}");
    }
    assert!(line(report, "settings").contains("0 permission rules, 0 hooks"));
    assert!(line(report, "trust").contains(&format!("{} is not trusted", sandbox.project().display())));
}

#[test]
fn an_untrusted_project_s_hooks_are_reported_as_skipped() {
    let sandbox = Sandbox::fresh();
    let hooks = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"true"}]}]}}"#;
    sandbox.write(&sandbox.project_dir().join("settings.json"), hooks);
    let ran = sandbox.run(&["doctor"]);
    let trust = line(&ran.stdout, "trust");
    assert!(trust.contains("1 project item skipped (Hooks in settings (1))"), "{ran:?}");
    assert!(trust.ends_with("/trust in the chat, --trust with --print"), "{ran:?}");
}

#[test]
fn a_broken_settings_file_fails_the_settings_check_naming_the_file() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.project_dir().join("settings.json");
    sandbox.write(&file, "{broken");
    let ran = sandbox.run(&["doctor"]);
    ran.failed_with(1).quiet_on_stderr();
    let settings = line(&ran.stdout, "settings");
    assert!(!passed(&ran.stdout, "settings"));
    assert!(settings.contains(&file.display().to_string()), "{ran:?}");
    assert!(settings.ends_with("(fix the file it names)"), "{ran:?}");
}

#[test]
fn a_declared_provider_that_does_not_load_is_reported_with_its_file() {
    let sandbox = Sandbox::fresh();
    let good = sandbox.user_dir().join("providers").join("loop.js");
    sandbox.write(
        &good,
        r#"export default { id: "loopback-test", name: "Loop", wire: "chat", base: "http://127.0.0.1:9/v1", models: ["m"] };"#,
    );
    let ran = sandbox.run(&["doctor"]);
    assert!(passed(&ran.stdout, "declared providers"), "{ran:?}");
    assert!(line(&ran.stdout, "declared providers").ends_with("1 loaded"), "{ran:?}");

    let broken = sandbox.user_dir().join("providers").join("broken.js");
    sandbox.write(&broken, "export default {");
    let ran = sandbox.run(&["doctor"]);
    assert!(!passed(&ran.stdout, "declared providers"), "{ran:?}");
    assert!(line(&ran.stdout, "declared providers").contains(&format!("{}: left out:", broken.display())));
}

#[test]
fn doctor_takes_no_arguments() {
    let ran = Sandbox::fresh().run(&["doctor", "extra"]);
    ran.failed_with(2).quiet_on_stdout().complained("unexpected argument 'extra'");
}
