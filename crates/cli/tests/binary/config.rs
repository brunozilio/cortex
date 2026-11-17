use crate::{Sandbox, command};

#[test]
fn settings_are_set_listed_read_and_unset_in_the_user_s_file() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.user_dir().join("settings.json");
    sandbox.run(&["config", "list"]).succeeded().quiet_on_stderr().said("{}");

    let set = sandbox.run(&["config", "set", "outputStyle", "concise"]);
    set.succeeded().quiet_on_stderr();
    assert_eq!(set.stdout.trim(), format!("Set outputStyle in {}.", file.display()));
    sandbox.run(&["config", "set", "permissions.allow", r#"["Bash(ls:*)"]"#]).succeeded();
    sandbox.run(&["config", "set", "cleanupPeriodDays", "30"]).succeeded();

    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("settings written")).expect("JSON");
    assert_eq!(
        kept,
        serde_json::json!({"outputStyle": "concise", "permissions": {"allow": ["Bash(ls:*)"]}, "cleanupPeriodDays": 30})
    );
    assert_eq!(sandbox.run(&["config", "list"]).succeeded().json(), kept);
    assert_eq!(
        sandbox.run(&["config", "get", "permissions.allow"]).succeeded().json(),
        serde_json::json!(["Bash(ls:*)"])
    );

    sandbox.run(&["config", "unset", "outputStyle"]).succeeded().said("Removed outputStyle from");
    let gone = sandbox.run(&["config", "get", "outputStyle"]);
    gone.failed_with(1).quiet_on_stdout().complained("outputStyle is not set in");
}

#[test]
fn reading_a_key_that_is_not_set_fails_naming_the_key_and_the_file() {
    let sandbox = Sandbox::fresh();
    let ran = sandbox.run(&["config", "get", "no.such.key"]);
    let file = sandbox.user_dir().join("settings.json");
    ran.failed_with(1).quiet_on_stdout();
    assert_eq!(ran.stderr.trim_end(), format!("{}: no.such.key is not set in {}", command(), file.display()));
}

#[test]
fn an_empty_key_is_refused() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["config", "set", "", "x"]).failed_with(1).quiet_on_stdout().complained("an empty key");
    assert!(!sandbox.user_dir().join("settings.json").exists(), "nothing was written");
}

#[test]
fn a_key_below_a_value_that_is_not_an_object_is_refused_naming_it() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["config", "set", "outputStyle", "concise"]).succeeded();
    let ran = sandbox.run(&["config", "set", "outputStyle.inner", "1"]);
    ran.failed_with(1).quiet_on_stdout().complained("outputStyle.inner");
    assert_eq!(sandbox.run(&["config", "get", "outputStyle"]).json(), "concise", "left as it was");
}

#[test]
fn project_settings_go_to_the_project_s_local_file_only() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["config", "--project", "set", "model", "small"]).succeeded();
    sandbox.run(&["config", "get", "model", "--project"]).succeeded().said("\"small\"");
    assert!(sandbox.project_dir().join("settings.local.json").is_file());
    assert!(!sandbox.user_dir().join("settings.json").exists());
    sandbox.run(&["config", "get", "model"]).failed_with(1).complained("model is not set");
}

#[test]
fn a_settings_file_that_is_not_json_is_named_in_the_error() {
    let sandbox = Sandbox::fresh();
    let file = sandbox.user_dir().join("settings.json");
    sandbox.write(&file, "{not json");
    for args in [["config", "list"].as_slice(), &["config", "set", "a", "b"]] {
        let ran = sandbox.run(args);
        ran.failed_with(1).quiet_on_stdout().complained(&format!("{}: {}: ", command(), file.display()));
    }
    assert_eq!(std::fs::read_to_string(&file).expect("kept"), "{not json", "the broken file is not overwritten");
}

#[test]
fn config_without_an_action_prints_its_usage_and_fails() {
    let ran = Sandbox::fresh().run(&["config"]);
    ran.failed_with(2).quiet_on_stdout().complained("Usage:").complained("unset");
}
